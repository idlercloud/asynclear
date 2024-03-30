use core::ptr::NonNull;

use crate::memory::{self, kernel_pa_to_va, ContinuousFrames, PhysAddr};

use super::BlockDevice;
use common::config::PA_TO_VA;
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
    fn dma_alloc(
        pages: usize,
        _direction: virtio_drivers::BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let frames = ContinuousFrames::alloc(pages).unwrap();
        // 这些 frames 交由库管理了，要阻止它调用 drop
        let pa_start = frames.start_ppn().page_start();
        core::mem::forget(frames);
        let vptr = NonNull::new(kernel_pa_to_va(pa_start).as_mut_ptr::<u8>()).unwrap();
        (pa_start.0, vptr)
    }

    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        _vaddr: core::ptr::NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let ppn = PhysAddr(paddr).ppn();
        unsafe {
            memory::frame_dealloc(ppn..(ppn + pages));
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, _size: usize) -> NonNull<u8> {
        let va = paddr + PA_TO_VA;
        NonNull::new(va as _).unwrap()
    }

    // 不知道 share 和 unshare 干嘛的，先这么实现着
    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        let vaddr = buffer.addr().get();
        assert!(vaddr >= PA_TO_VA);
        vaddr - PA_TO_VA
    }

    // 在我们的场景中似乎不需要？
    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) {
    }
}

impl BlockDevice for VirtIOBlk<HalImpl, MmioTransport> {
    const BLOCK_SIZE: u32 = 512;

    fn read_block(&mut self, block_id: u64, buf: &mut [u8; Self::BLOCK_SIZE as usize]) {
        self.read_blocks(block_id as usize, buf)
            .expect("Error when reading VirtIOBlk");
    }
    fn write_block(&mut self, block_id: u64, buf: &[u8; Self::BLOCK_SIZE as usize]) {
        self.write_blocks(block_id as usize, buf)
            .expect("Error when writing VirtIOBlk");
    }
}

/// `BLOCK_SIZE` 必须是 `SECTOR_SIZE` 的正整数倍
const _: () = assert!(
    (<VirtIOBlk<HalImpl, MmioTransport> as BlockDevice>::BLOCK_SIZE as usize)
        % virtio_drivers::device::blk::SECTOR_SIZE
        == 0
);

pub fn new() -> VirtIOBlk<HalImpl, MmioTransport> {
    const VIRTIO0: usize = 0x10001000;
    let header = NonNull::new((VIRTIO0 + PA_TO_VA) as *mut VirtIOHeader).unwrap();
    let transport = unsafe { MmioTransport::new(header).unwrap() };
    assert!(transport.device_type() == DeviceType::Block);
    VirtIOBlk::new(transport).unwrap()
}
