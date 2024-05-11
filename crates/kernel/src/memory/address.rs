use core::{
    iter::Step,
    ops::{Add, Sub},
};

use common::config::{PAGE_OFFSET_MASK, PAGE_SIZE, PAGE_SIZE_BITS, PTE_PER_PAGE};

/// 物理地址。在 Sv39 页表机制中，虚拟地址转化得到的物理地址总共为 56 位，其中页号 44 位，页内偏移 12 位。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct PhysAddr(pub usize);

impl PhysAddr {
    /// 向下取整页号
    pub const fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0.div_floor(PAGE_SIZE))
    }

    /// 向上取整页号
    pub const fn ceil(&self) -> PhysPageNum {
        PhysPageNum(self.0.div_ceil(PAGE_SIZE))
    }

    pub const fn ppn(&self) -> PhysPageNum {
        self.floor()
    }
}

impl Add<usize> for PhysAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

/// 物理页号。Sv39 中合法的页号只考虑低 44 位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysPageNum(pub usize);

impl PhysPageNum {
    pub fn page_start(self) -> PhysAddr {
        PhysAddr(self.0 << PAGE_SIZE_BITS)
    }
}

impl Add<usize> for PhysPageNum {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Step for PhysPageNum {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        end.0.checked_sub(start.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_add(count).map(PhysPageNum)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_sub(count).map(PhysPageNum)
    }
}

/// 虚拟地址。在 Sv39 页表机制中，虚拟地址 38~0 有效，39 及高位和 38 位一致，页号 27 位，页内偏移 12 位。
///
/// 由于 63~39 和 38 位保持一致，虚拟地址空间中只有 64 位的最低 256 GB 地址和最高 256 GB 地址有效。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct VirtAddr(pub usize);

impl VirtAddr {
    pub const fn page_offset(&self) -> usize {
        self.0 & PAGE_OFFSET_MASK
    }

    /// 向下取整页号
    pub const fn vpn_floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 >> PAGE_SIZE_BITS)
    }

    /// 向上取整页号
    pub const fn vpn_ceil(&self) -> VirtPageNum {
        VirtPageNum(self.0.div_ceil(PAGE_SIZE))
    }

    /// # Safety
    ///
    /// 需要保证该地址转化为 T 后内容合法
    #[track_caller]
    pub unsafe fn as_ref<'a, T>(&self) -> &'a T {
        unsafe { (self.0 as *const T).as_ref().unwrap() }
    }

    /// # Safety
    ///
    /// 需要保证该地址转化为 T 后内容合法
    #[track_caller]
    pub unsafe fn as_mut<'a, T>(&mut self) -> &'a mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

impl<T> From<*const T> for VirtAddr {
    fn from(ptr: *const T) -> Self {
        Self(ptr as usize)
    }
}

impl<T> From<*mut T> for VirtAddr {
    fn from(ptr: *mut T) -> Self {
        Self(ptr as usize)
    }
}

/// 虚拟页号。应满足：仅低 27 位有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtPageNum(pub usize);

impl VirtPageNum {
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0; 3];
        for i in idx.iter_mut().rev() {
            const LOW_MASK: usize = PTE_PER_PAGE - 1;
            *i = vpn & LOW_MASK;
            vpn >>= 9;
        }
        idx
    }

    pub fn page_start(&self) -> VirtAddr {
        VirtAddr(self.0 << PAGE_SIZE_BITS)
    }

    /// # Safety
    ///
    /// 任何页都可以转化为字节数组。但可能造成 alias，所以先标为 `unsafe`
    pub unsafe fn as_page_bytes_mut<'a>(&mut self) -> &'a mut [u8; PAGE_SIZE] {
        unsafe { self.page_start().as_mut() }
    }

    pub fn with_offset(self, offset: usize) -> VirtAddr {
        VirtAddr(self.page_start().0 + offset)
    }
}

impl Add<usize> for VirtPageNum {
    type Output = Self;

    fn add(self, len: usize) -> Self::Output {
        Self(self.0 + len)
    }
}

impl Sub<usize> for VirtPageNum {
    type Output = Self;

    fn sub(self, len: usize) -> Self::Output {
        Self(self.0 - len)
    }
}

impl Step for VirtPageNum {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        end.0.checked_sub(start.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_add(count).map(VirtPageNum)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.0.checked_sub(count).map(VirtPageNum)
    }
}
