//! Implementation of [`FrameAllocator`] which
//! controls all the frames in the operating system.

use core::ops::Range;

use super::address::PhysAddr;

use super::{kernel_ppn_to_vpn, kernel_va_to_pa, PhysPageNum, VirtAddr};
use common::config::{MEMORY_END, MEMORY_SIZE, PAGE_SIZE};
use klocks::SpinMutex;

#[derive(Debug)]
pub struct Frame {
    ppn: PhysPageNum,
}

impl Frame {
    pub fn alloc() -> Option<Self> {
        let ppn = FRAME_ALLOCATOR.lock().alloc(1)?;
        let mut frame = Self { ppn };
        frame.fill(0);
        Some(frame)
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.ppn
    }

    fn fill(&mut self, byte: u8) {
        let mut va = kernel_ppn_to_vpn(self.ppn).page_start();
        unsafe {
            va.as_mut::<[u8; PAGE_SIZE]>().fill(byte);
        }
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
        frames.fill(0);
        Some(frames)
    }

    pub fn start_ppn(&self) -> PhysPageNum {
        self.ppn
    }

    fn fill(&mut self, byte: u8) {
        let va = kernel_ppn_to_vpn(self.ppn).page_start();
        unsafe {
            let bytes = core::slice::from_raw_parts_mut(va.0 as _, self.num * PAGE_SIZE);
            bytes.fill(byte);
        }
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
        let physical_memory_begin_frame: usize =
            kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
        self.allocator
            .alloc(num)
            .map(|first| PhysPageNum(first + physical_memory_begin_frame))
    }

    unsafe fn dealloc(&mut self, range: Range<PhysPageNum>) {
        let physical_memory_begin_frame: usize =
            kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
        self.allocator.dealloc(
            range.start.0 - physical_memory_begin_frame,
            range.end.0 - range.start.0,
        );
    }
}

type FrameAllocatorImpl = BuddySystemFrameAllocator;

static FRAME_ALLOCATOR: SpinMutex<FrameAllocatorImpl> = SpinMutex::new(FrameAllocatorImpl::new());

pub fn init_frame_allocator() {
    let physical_memory_begin_frame = kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
    FRAME_ALLOCATOR.lock().allocator.add_frame(
        0,
        PhysAddr(MEMORY_END).floor().0 - physical_memory_begin_frame,
    );
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
