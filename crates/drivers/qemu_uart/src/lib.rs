#![no_std]

mod tty;

#[macro_use]
extern crate kernel_tracer;

use defines::config::{PA_TO_VA, QEMU_UART_ADDR};
use heapless::Deque;
use klocks::{Lazy, SpinNoIrqMutex};
use tty::Tty;
use uart_16550::MmioSerialPort;

pub static UART0: Lazy<Uart> = Lazy::new(|| {
    let mut port = unsafe { MmioSerialPort::new(PA_TO_VA + QEMU_UART_ADDR) };
    port.init();

    Uart {
        port: SpinNoIrqMutex::new(port),
    }
});

pub static TTY: SpinNoIrqMutex<Tty> = SpinNoIrqMutex::new(Tty {
    queue: Deque::new(),
    waker: None,
});

pub struct Uart {
    port: SpinNoIrqMutex<MmioSerialPort>,
}

impl Uart {
    pub fn handle_irq(&self) {
        trace!("uart interrupt");
        let ch = self.port.lock().receive();
        let mut tty = TTY.lock();
        if tty.queue.push_back(ch).is_err() {
            trace!("uart input discard: {ch}");
        }
        if let Some(waker) = tty.waker.take() {
            waker.wake();
        }
    }
}
