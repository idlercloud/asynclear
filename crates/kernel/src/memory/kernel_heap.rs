use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use buddy_system_allocator::Heap;
use common::config::KERNEL_HEAP_SIZE;
use klocks::SpinNoIrqMutex;

// TODO: `buddy_system_allocator` 的 order 直接设为 32 了，是否有可能超出了，对性能会有影响吗？
#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap(SpinNoIrqMutex::new(Heap::<32>::new()));

pub struct LockedHeap<const ORDER: usize>(SpinNoIrqMutex<Heap<ORDER>>);

unsafe impl<const ORDER: usize> GlobalAlloc for LockedHeap<ORDER> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .lock()
            .dealloc(unsafe { NonNull::new_unchecked(ptr) }, layout);
    }
}

/// 实际上的内核堆空间
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// 初始化内核堆，只应当调用一次
pub unsafe fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR
            .0
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}
