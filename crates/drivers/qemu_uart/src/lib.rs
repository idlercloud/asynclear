#![no_std]

use core::task::Waker;

use async_lock::{Mutex, MutexGuard};
use common::config::{PA_TO_VA, QEMU_UART_ADDR};
use futures::task::AtomicWaker;
use heapless::{spsc, Deque};
use klocks::{Lazy, SpinNoIrqMutex};
use uart_16550::MmioSerialPort;

// INFO: 中断体系会确保只有一个 hart 在进行中断处理

pub static UART0: Lazy<Uart> = Lazy::new(|| {
    let mut port = unsafe { MmioSerialPort::new(PA_TO_VA + QEMU_UART_ADDR) };
    port.init();
    static mut UART_BUFFER: spsc::Queue<u8, UART_BUFFER_LEN> = spsc::Queue::new();

    #[allow(static_mut_refs)]
    let (producer, consumer) = unsafe { UART_BUFFER.split_const() };

    Uart {
        port: SpinNoIrqMutex::new(port),
        producer: SpinNoIrqMutex::new(producer),
        consumer: Mutex::new(consumer),
        waker: AtomicWaker::new(),
    }
});

pub struct Uart {
    port: SpinNoIrqMutex<MmioSerialPort>,
    producer: SpinNoIrqMutex<spsc::Producer<'static, u8>>,
    consumer: Mutex<spsc::Consumer<'static, u8>>,
    pub waker: AtomicWaker,
}

impl Uart {
    pub fn print(&self, s: &str) {
        let mut port = self.port.lock();
        for byte in s.as_bytes() {
            port.send(*byte);
        }
    }

    pub fn handle_irq(&self) {
        let mut uart_port = self.port.lock();
        let mut producer = self.producer.lock();
        while let Ok(ch) = uart_port.try_receive() {
            // NOTE: 如果超出容量则会静默舍弃
            let _ = producer.enqueue(ch);
        }
        self.waker.wake();
    }

    pub async fn lock_consumer(&self) -> MutexGuard<'_, spsc::Consumer<'static, u8>> {
        self.consumer.lock().await
    }
}

const UART_BUFFER_LEN: usize = 128;

pub struct UartBuffer {
    pub(crate) queue: Deque<u8, UART_BUFFER_LEN>,
    // TODO: 这里似乎可以用 futures::AtomicWaker 代替？
    pub(crate) waker: Option<Waker>,
}

impl UartBuffer {
    pub fn pop_byte(&mut self) -> Option<u8> {
        self.queue.pop_front()
    }

    pub fn clear_buffer(&mut self) {
        self.queue.clear();
    }

    /// 注册一个任务。如果原来已经注册过，就将旧的任务唤醒。
    ///
    /// 这是为了防止多个进程读时永久阻塞不被唤醒
    pub fn register_waker(&mut self, waker: Waker) {
        if let Some(old_waker) = self.waker.replace(waker) {
            old_waker.wake();
        }
    }
}
