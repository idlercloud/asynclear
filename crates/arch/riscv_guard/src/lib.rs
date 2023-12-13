#![no_std]

use riscv::register::sstatus;

pub struct NoIrqGuard {
    before: bool,
}

impl NoIrqGuard {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let before = sstatus::read().sie();
        if before {
            unsafe {
                sstatus::clear_sie();
            }
        }
        Self { before }
    }
}

impl Drop for NoIrqGuard {
    fn drop(&mut self) {
        if self.before {
            unsafe {
                sstatus::set_sie();
            }
        }
    }
}
