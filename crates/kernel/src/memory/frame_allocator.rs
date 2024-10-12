//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use core::{mem::ManuallyDrop, ops::Range};

use common::config::{MEMORY_END, MEMORY_SIZE, PAGE_SIZE};
use klocks::SpinMutex;

use super::{address::PhysAddr, kernel_ppn_to_vpn, kernel_va_to_pa, PhysPageNum, VirtAddr};

#[derive(Debug)]
pub struct Frame {
    ppn: PhysPageNum,
}

impl Frame {
    pub fn alloc() -> Option<Self> {
        let ppn = FRAME_ALLOCATOR.lock().alloc(1)?;
        let mut frame = Self { ppn };
        frame.clear();
        Some(frame)
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.ppn
    }

    /// 直接用 ppn 构建一个不拥有所有权的 Frame 视图
    ///
    /// # SAFETY
    ///
    /// 需保证 ppn 指向的物理页已被分配且未 alias
    pub unsafe fn view(ppn: PhysPageNum) -> ManuallyDrop<Self> {
        ManuallyDrop::new(Self { ppn })
    }

    fn clear(&mut self) {
        self.as_page_bytes_mut().fill(0);
    }

    pub fn copy_from(&mut self, src: &Self) {
        self.as_page_bytes_mut().copy_from_slice(src.as_page_bytes());
    }

    /// # SAFETY
    ///
    /// 需保证类型的 alignment，并且不会越过页边界
    pub unsafe fn as_ref_at<'a, T>(&self, offset: usize) -> &'a T {
        unsafe { kernel_ppn_to_vpn(self.ppn).with_offset(offset).as_ref() }
    }

    /// # SAFETY
    ///
    /// 需保证类型的 alignment，并且不会越过页边界
    pub unsafe fn as_mut_at<'a, T>(&mut self, offset: usize) -> &'a mut T {
        unsafe { kernel_ppn_to_vpn(self.ppn).with_offset(offset).as_mut() }
    }

    pub fn as_page_bytes(&self) -> &[u8; PAGE_SIZE] {
        unsafe { self.as_ref_at(0) }
    }

    pub fn as_page_bytes_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        unsafe { self.as_mut_at(0) }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            frame_dealloc(self.ppn..(self.ppn + 1));
        }
    }
}

#[derive(Debug)]
pub struct ContinuousFrames {
    pub ppn: PhysPageNum,
    pub num: usize,
}

impl ContinuousFrames {
    /// 分配并清空一段连续的物理页帧
    pub fn alloc(num: usize) -> Option<Self> {
        debug_assert!(num >= 1);
        let ppn = FRAME_ALLOCATOR.lock().alloc(num)?;
        let mut frames = Self { ppn, num };
        frames.clear();
        Some(frames)
    }

    fn clear(&mut self) {
        let va = kernel_ppn_to_vpn(self.ppn).page_start();
        unsafe {
            let bytes = core::slice::from_raw_parts_mut(va.0 as _, self.num * PAGE_SIZE);
            bytes.fill(0);
        }
    }

    pub fn start_ppn(&self) -> PhysPageNum {
        self.ppn
    }
}

impl Drop for ContinuousFrames {
    fn drop(&mut self) {
        unsafe {
            frame_dealloc(self.ppn..(self.ppn + self.num));
        }
    }
}

trait FrameAllocator {
    fn alloc(&mut self, num: usize) -> Option<PhysPageNum>;
    unsafe fn dealloc(&mut self, range: Range<PhysPageNum>);
}

const BUDDY_ORDER: usize = ((MEMORY_SIZE - 1) / PAGE_SIZE).ilog2() as usize + 1;

pub struct BuddySystemFrameAllocator {
    allocator: buddy_system_allocator::FrameAllocator<BUDDY_ORDER>,
}

impl BuddySystemFrameAllocator {
    pub const fn new() -> Self {
        Self {
            allocator: buddy_system_allocator::FrameAllocator::new(),
        }
    }
}

extern "C" {
    fn ekernel();
}

impl FrameAllocator for BuddySystemFrameAllocator {
    fn alloc(&mut self, num: usize) -> Option<PhysPageNum> {
        let physical_memory_begin_frame: usize = kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
        self.allocator
            .alloc(num)
            .map(|first| PhysPageNum(first + physical_memory_begin_frame))
    }

    unsafe fn dealloc(&mut self, range: Range<PhysPageNum>) {
        let physical_memory_begin_frame: usize = kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
        self.allocator
            .dealloc(range.start.0 - physical_memory_begin_frame, range.end.0 - range.start.0);
    }
}

type FrameAllocatorImpl = BuddySystemFrameAllocator;

static FRAME_ALLOCATOR: SpinMutex<FrameAllocatorImpl> = SpinMutex::new(FrameAllocatorImpl::new());

pub fn init_frame_allocator() {
    let physical_memory_begin_frame = kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
    FRAME_ALLOCATOR
        .lock()
        .allocator
        .add_frame(0, PhysAddr(MEMORY_END).floor().0 - physical_memory_begin_frame);
}

/// # Safety
///
/// 需要保证 range 内的物理页之前都实际被分配
#[track_caller]
pub unsafe fn frame_dealloc(range: Range<PhysPageNum>) {
    unsafe {
        FRAME_ALLOCATOR.lock().dealloc(range);
    }
}
