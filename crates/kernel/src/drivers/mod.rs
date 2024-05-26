pub mod qemu_block;
pub mod qemu_plic;
pub mod qemu_uart;

use common::config;
use klocks::Lazy;

use self::{qemu_block::BLOCK_DEVICE, qemu_plic::Plic, qemu_uart::UART0};

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

    Lazy::force(&UART0);
    Lazy::force(&BLOCK_DEVICE);

    unsafe {
        riscv::register::sie::set_sext();
    }
}
