// TODO: 虽然名字是 hal，实际上却只是各种 drivers 的一个聚合罢了

#![no_std]
#![allow(incomplete_features)]
#![feature(strict_provenance)]
#![feature(generic_const_exprs)]

pub use qemu_block::DiskDriver;
pub use qemu_plic::Plic;
pub use qemu_uart::UART0;

use klocks::Lazy;

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
    for context in 0..(defines::config::HART_NUM * 2) {
        plic.set_threshold(context, 0);
        plic.enable(InterruptSource::Uart0 as usize, context);
        // plic.enable(InterruptSource::VirtIO as usize, context);
    }
    plic.set_priority(InterruptSource::Uart0 as usize, 1);
    // plic.set_priority(InterruptSource::VirtIO as usize, 1);

    Lazy::force(&UART0);

    unsafe {
        riscv::register::sie::set_sext();
    }
}
