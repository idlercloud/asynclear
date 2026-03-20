use core::ptr::NonNull;

use common::config::{self, PA_TO_VA};
use console_output::eprintln;
use fdt::{node::FdtNode, Fdt};
use klocks::{Lazy, Once};
use libkernel::memory::{self, MapPermission, VirtAddr, KERNEL_SPACE};
use qemu_plic::Plic;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{
        mmio::{MmioError, MmioTransport},
        DeviceType, DeviceTypeError, Transport,
    },
};
use virtio_glue::{DiskDriver, HalImpl};

pub enum InterruptSource {
    VirtIO = 1,
    Uart0 = 10,
}

impl InterruptSource {
    pub fn from_id(id: usize) -> Option<Self> {
        match id {
            1 => Some(Self::VirtIO),
            10 => Some(Self::Uart0),
            _ => None,
        }
    }
}

pub fn init(fdt: &Fdt<'_>) {
    let plic = unsafe { &(*Plic::mmio()) };
    for context in 0..(config::MAX_HART_NUM * 2) {
        plic.set_threshold(context, 0);
        plic.enable(InterruptSource::Uart0 as usize, context);
        // plic.enable(InterruptSource::VirtIO as usize, context);
    }
    plic.set_priority(InterruptSource::Uart0 as usize, 1);
    // plic.set_priority(InterruptSource::VirtIO as usize, 1);

    for node in fdt.all_nodes() {
        try_probe_virtio(node);
    }
    let Some(block_device) = BLOCK_DEVICE.get() else {
        panic!("No boot block device");
    };
    hal::block_device::init_instance(block_device);

    Lazy::force(&qemu_uart::UART0);
}

static BLOCK_DEVICE: Once<DiskDriver<HalImpl, MmioTransport<'static>>> = Once::new();

fn try_probe_virtio(node: FdtNode<'_, '_>) {
    if !node.compatible().is_some_and(|c| c.all().any(|s| s == "virtio,mmio")) {
        return;
    }

    let Some(reg) = node.reg().and_then(|mut regs| regs.next()) else {
        return;
    };

    let paddr = reg.starting_address as usize;
    let size = reg.size.unwrap();
    let vaddr = VirtAddr(paddr + PA_TO_VA);
    // SAFETY: 初始化设备时只有主核在运行，且内核空间映射已经完成
    let kernel_space = KERNEL_SPACE.as_mut_ptr();
    unsafe {
        (*kernel_space).kernel_map(
            vaddr,
            vaddr + size,
            MapPermission::R | MapPermission::W | MapPermission::G,
        );
        memory::flush_tlb_range(vaddr, size);
    }
    let header = NonNull::new(vaddr.as_mut_ptr()).unwrap();
    match unsafe { MmioTransport::new(header, size) } {
        Ok(transport) => {
            let device_type = transport.device_type();
            eprintln!(
                "Detected virtio MMIO device with vendor id {:#X}, device type {:?}, version {:?}",
                transport.vendor_id(),
                device_type,
                transport.version(),
            );
            match device_type {
                DeviceType::Block => probe_virtio_blk(transport),
                _ => {}
            }
        }
        // 无效设备类型，忽略
        Err(MmioError::InvalidDeviceID(DeviceTypeError::InvalidDeviceType(0))) => {}
        Err(e) => eprintln!("Error creating VirtIO MMIO transport: {}", e),
    }
}

fn probe_virtio_blk(transport: MmioTransport<'static>) {
    let blk = VirtIOBlk::<HalImpl, MmioTransport<'static>>::new(transport).expect("failed to create blk driver");
    let block_device = DiskDriver::new(blk);
    BLOCK_DEVICE.call_once(|| block_device);
}
