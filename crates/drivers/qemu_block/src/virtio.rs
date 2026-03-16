use core::ptr::NonNull;

use common::config::{PA_TO_VA, QEMU_VIRTIO0};
use libkernel::memory::{self, ContinuousFrames, PhysAddr};
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
    Hal,
};

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: virtio_drivers::BufferDirection) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
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
    unsafe fn share(buffer: NonNull<[u8]>, _direction: virtio_drivers::BufferDirection) -> virtio_drivers::PhysAddr {
        let vaddr = buffer.as_ptr() as *const u8 as usize;
        assert!(vaddr >= PA_TO_VA);
        (vaddr - PA_TO_VA) as u64
    }

    // 在我们的场景中似乎不需要？
    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) {
    }
}

pub fn init<'a>() -> VirtIOBlk<HalImpl, MmioTransport<'a>> {
    let header = NonNull::new((QEMU_VIRTIO0 + PA_TO_VA) as *mut VirtIOHeader).unwrap();
    // TODO: MmioTransport::new 的第二个参数 mmio_size 不知道是多少。之后改为从 ftb 中读取
    let transport = unsafe { MmioTransport::new(header, 0x200).unwrap() };
    assert!(transport.device_type() == DeviceType::Block);
    VirtIOBlk::new(transport).unwrap()
}
