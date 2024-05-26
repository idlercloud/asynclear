mod tty;

use common::config::{PA_TO_VA, QEMU_UART_ADDR};
use heapless::Deque;
use klocks::{Lazy, SpinNoIrqMutex};
use tty::Tty;
use uart_16550::MmioSerialPort;

// FIXME: 关中断锁只对一个 hart 生效，因此仍然可能导致中断处理有不可预测的延迟乃至死锁

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
    pub fn print(&self, s: &str) {
        let mut port = self.port.lock();
        for byte in s.as_bytes() {
            port.send(*byte);
        }
    }

    pub fn handle_irq(&self) {
        let ch = self.port.lock().receive();
        let mut tty = TTY.lock();
        let _ = tty.queue.push_back(ch);
        if let Some(waker) = tty.waker.take() {
            waker.wake();
        }
    }
}
