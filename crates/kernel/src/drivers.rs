use common::config;
use klocks::Lazy;
use qemu_plic::Plic;

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

pub fn init() {
    let plic = unsafe { &(*Plic::mmio()) };
    for context in 0..(config::MAX_HART_NUM * 2) {
        plic.set_threshold(context, 0);
        plic.enable(InterruptSource::Uart0 as usize, context);
        // plic.enable(InterruptSource::VirtIO as usize, context);
    }
    plic.set_priority(InterruptSource::Uart0 as usize, 1);
    // plic.set_priority(InterruptSource::VirtIO as usize, 1);

    Lazy::force(&qemu_uart::UART0);
    hal::block_device::init_instance(Lazy::force(&qemu_block::BLOCK_DEVICE) as _);
}
