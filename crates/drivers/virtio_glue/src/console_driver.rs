use core::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use klocks::SpinNoIrqMutex;
use virtio_drivers::{device::console::VirtIOConsole, transport::Transport, Hal};

pub struct ConsoleDriver<H: Hal, T: Transport> {
    device: SpinNoIrqMutex<VirtIOConsole<H, T>>,
    enabled: AtomicBool,
}

impl<H: Hal, T: Transport> ConsoleDriver<H, T> {
    pub fn new(device: VirtIOConsole<H, T>) -> Self {
        Self {
            device: SpinNoIrqMutex::new(device),
            enabled: AtomicBool::new(true),
        }
    }

    pub fn write_with(&self, write: impl FnOnce(&mut VirtIOConsole<H, T>) -> fmt::Result) -> fmt::Result {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(());
        }

        let result = write(&mut self.device.lock());
        if result.is_err() {
            self.enabled.store(false, Ordering::Release);
        }
        result
    }
}
