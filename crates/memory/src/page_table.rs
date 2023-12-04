//! Implementation of [`PageTableEntry`] and [`PageTable`].

use crate::kernel_pa_to_va;

use super::{
    frame_alloc, kernel_ppn_to_vpn, FrameTracker, MapPermission, PhysAddr, PhysPageNum, VirtAddr,
    VirtPageNum,
};
use alloc::{vec, vec::Vec};
use bitflags::*;
use defines::{
    config::PAGE_SIZE,
    error::{errno, Result},
};
use riscv::register::satp;

bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

impl From<MapPermission> for PTEFlags {
    fn from(mp: MapPermission) -> Self {
        Self::from_bits_truncate(mp.bits())
    }
}

/// page table entry structure
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits() as usize,
        }
    }
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    pub fn ppn(&self) -> PhysPageNum {
        const LOW_44_MASK: usize = (1 << 44) - 1;
        PhysPageNum((self.bits >> 10) & LOW_44_MASK)
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits as u8)
    }
    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }
}

/// 页表，其内跟踪了页表所占用的帧，页表释放时，释放这些帧
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// 假定创建和映射时不会导致内存不足
impl PageTable {
    /// 注意，创建时会分配一个根页表的帧
    pub fn with_root() -> Self {
        let frame = frame_alloc(1).unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    /// 清除根页表之外的其他页表。一般而言，用户的页表中只会含有低地址的页表
    pub fn clear_except_root(&mut self) {
        self.frames.truncate(1);
    }

    /// # Safety
    ///
    /// 请自行保证 non-alias
    pub unsafe fn root_pte(&self, line: usize) -> &PageTableEntry {
        unsafe { &kernel_ppn_to_vpn(self.root_ppn).as_page_ptes()[line] }
    }

    /// # Safety
    ///
    /// 请自行保证 non-alias
    pub unsafe fn root_pte_mut(&mut self, line: usize) -> &mut PageTableEntry {
        unsafe { &mut kernel_ppn_to_vpn(self.root_ppn).as_page_ptes_mut()[line] }
    }

    /// # Safety
    ///
    /// 请自行保证 non-alias
    pub unsafe fn root_page(&mut self) -> &mut [u8; PAGE_SIZE] {
        unsafe { kernel_ppn_to_vpn(self.root_ppn).as_page_bytes_mut() }
    }

    /// 找到 `vpn` 对应的叶子页表项。注意，该页表项必须是 valid 的。
    ///
    /// TODO: 是否要将翻译相关的函数返回值改为 Result？
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut ret = None;
        for idx in idxs {
            // 因为从 root_ppn 开始，只要保证 root_ppn 合法则 pte 合法
            let pte = unsafe { &kernel_ppn_to_vpn(ppn).as_page_ptes()[idx] };
            if !pte.is_valid() {
                return None;
            }
            ret = Some(pte);
            ppn = pte.ppn();
        }
        ret
    }

    /// 找到 `vpn` 对应的叶子页表项。注意不保证该页表项 valid，需调用方自己修改
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut ret: Option<&mut PageTableEntry> = None;
        for (i, &idx) in idxs.iter().enumerate() {
            // 因为从 root_ppn 开始，只要保证 root_ppn 合法则 pte 合法
            let pte = unsafe { &mut kernel_ppn_to_vpn(ppn).as_page_ptes_mut()[idx] };
            // 这里假定为 3 级页表
            if i == 2 {
                ret = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc(1).unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        ret
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(
            !pte.is_valid(),
            "vpn {:#x?} is mapped before mapping",
            vpn.0
        );
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {vpn:?} is invalid before unmapping");
        *pte = PageTableEntry::empty();
    }

    #[inline]
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PhysPageNum> {
        self.find_pte(vpn).copied().map(|pte| pte.ppn())
    }

    #[inline]
    pub fn trans_va_to_pa(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.vpn_floor()).map(|pte| {
            let aligned_pa = pte.ppn().page_start();
            aligned_pa + va.page_offset()
        })
    }

    /// 将用户指针转换到内核的虚拟地址。
    #[inline]
    #[track_caller]
    pub fn trans_va(&self, va: VirtAddr) -> Result<VirtAddr> {
        self.trans_va_to_pa(va)
            .map(kernel_pa_to_va)
            .ok_or(errno::EFAULT)
    }

    #[inline]
    pub fn token(&self) -> usize {
        (satp::Mode::Sv39 as usize) << 60 | self.root_ppn.0
    }
}
