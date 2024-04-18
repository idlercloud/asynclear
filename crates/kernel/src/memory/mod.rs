mod address;
mod frame_allocator;
mod kernel_heap;
mod memory_space;
mod page;

use common::config::{PAGE_SIZE, PA_TO_VA};

pub use self::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
pub use self::frame_allocator::{frame_dealloc, ContinuousFrames};
pub use self::memory_space::{
    flush_tlb, log_kernel_sections,
    page_table::{PTEFlags, PageTable},
    vm_area::{AreaType, FramedVmArea},
    MapPermission, MemorySpace, KERNEL_SPACE,
};
pub use self::page::Page;

#[inline]
const fn kernel_va_to_pa(va: VirtAddr) -> PhysAddr {
    PhysAddr(va.0 - PA_TO_VA)
}

#[inline]
pub const fn kernel_pa_to_va(pa: PhysAddr) -> VirtAddr {
    VirtAddr(pa.0 + PA_TO_VA)
}

#[inline]
const fn kernel_vpn_to_ppn(vpn: VirtPageNum) -> PhysPageNum {
    PhysPageNum(vpn.0 - PA_TO_VA / PAGE_SIZE)
}

#[inline]
const fn kernel_ppn_to_vpn(ppn: PhysPageNum) -> VirtPageNum {
    VirtPageNum(ppn.0 + PA_TO_VA / PAGE_SIZE)
}

/// 初始化内存模块，包括内核堆、帧分配器
///
/// # Safety
///
/// 只应当调用一次
pub unsafe fn init() {
    unsafe { kernel_heap::init_heap() };
    frame_allocator::init_frame_allocator();
}
