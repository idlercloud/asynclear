use alloc::boxed::Box;
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
    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> virtio_drivers::PhysAddr {
        // 有可能 buffer 是来自用户地址空间的，比如 log 会走到 log_to_file 最终走到这里
        // TODO: 等实现 copy_from_user 后应该不再需要特殊处理
        let len = buffer.len();
        let mut shared = Box::<[u8]>::new_uninit_slice(len);
        if let BufferDirection::DriverToDevice | BufferDirection::Both = direction {
            unsafe {
                shared
                    .as_mut_ptr()
                    .cast::<u8>()
                    .copy_from(buffer.as_ptr().cast::<u8>(), len);
            }
        }
        Box::into_raw(shared).cast::<u8>() as u64 - PA_TO_VA as u64
    }

    unsafe fn unshare(paddr: virtio_drivers::PhysAddr, buffer: NonNull<[u8]>, direction: BufferDirection) {
        let vaddr = paddr + PA_TO_VA as u64;
        let len = buffer.len();
        let shared = unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(vaddr as *mut u8, len)) };
        if let BufferDirection::DeviceToDriver | BufferDirection::Both = direction {
            unsafe {
                buffer
                    .as_ptr()
                    .cast::<u8>()
                    .copy_from(shared.as_ptr().cast::<u8>(), len);
            }
        }
    }
}
