use core::ptr::NonNull;

use common::config::PA_TO_VA;
use libkernel::memory::{self, ContinuousFrames, PhysAddr};
use virtio_drivers::{BufferDirection, Hal};

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let frames = ContinuousFrames::alloc(pages).unwrap();
        // 这些 frames 交由库管理了，要阻止它调用 drop
        let pa_start = frames.start_ppn().page_start();
        core::mem::forget(frames);
        let vptr = NonNull::new(memory::kernel_pa_to_va(pa_start).as_mut_ptr::<u8>()).unwrap();
        (pa_start.0 as u64, vptr)
    }

    unsafe fn dma_dealloc(paddr: virtio_drivers::PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let ppn = PhysAddr(paddr as usize).ppn();
        unsafe {
            memory::frame_dealloc(ppn..(ppn + pages));
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, _size: usize) -> NonNull<u8> {
        let va = paddr + PA_TO_VA as u64;
        NonNull::new(va as _).unwrap()
    }

    // 不知道 share 和 unshare 干嘛的，先这么实现着
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> virtio_drivers::PhysAddr {
        let vaddr = buffer.as_ptr() as *const u8 as usize;
        assert!(vaddr >= PA_TO_VA);
        (vaddr - PA_TO_VA) as u64
    }

    // 在我们的场景中似乎不需要？
    unsafe fn unshare(_paddr: virtio_drivers::PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}
