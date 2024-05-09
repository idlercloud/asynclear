#![no_std]
#![feature(negative_impls)]

// FIXME: 这些 guard 都要求 guard
// 的创建和析构呈严格栈结构，否则会有问题，需要改进

use riscv::register::sstatus;

pub struct NoIrqGuard {
    before: bool,
}

// 不允许 Guard 越过 .await
impl !Send for NoIrqGuard {}

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

pub struct AccessUserGuard {
    before: bool,
}

// 不允许 Guard 越过 .await
impl !Send for AccessUserGuard {}

impl AccessUserGuard {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let before = sstatus::read().sum();
        if !before {
            unsafe {
                sstatus::set_sum();
            }
        }
        Self { before }
    }
}

impl Drop for AccessUserGuard {
    fn drop(&mut self) {
        if !self.before {
            unsafe {
                sstatus::clear_sum();
            }
        }
    }
}
