use crate::SignalFlag;

use super::SignalSet;
use bitflags::bitflags;

/// 跨越 syscall 边界的结构体，仅用于和 Linux 定义一致
#[repr(C)]
#[derive(Clone, Debug)]
pub struct SignalAction {
    /// singal handler 的地址
    pub handler: usize,
    /// 信号处理程序运行期间，额外的掩码
    pub mask: SignalSet,
    pub flags: SignalActionFlags,
    pub restorer: usize,
}

impl From<&KSignalAction> for SignalAction {
    fn from(value: &KSignalAction) -> Self {
        Self {
            handler: value.handler,
            mask: SignalSet::with_flag(value.mask),
            flags: value.flags,
            restorer: value.restorer,
        }
    }
}

/// 内核中真正存储的 signal action。它无需是 `#[repr(C)]`，而且 mask 只存了实际需要的
#[derive(Clone, Debug)]
pub struct KSignalAction {
    /// singal handler 的地址
    handler: usize,
    mask: SignalFlag,
    flags: SignalActionFlags,
    restorer: usize,
}

impl From<&SignalAction> for KSignalAction {
    fn from(value: &SignalAction) -> Self {
        Self {
            handler: value.handler,
            mask: value.mask.flag(),
            flags: value.flags,
            restorer: value.restorer,
        }
    }
}

impl KSignalAction {
    pub const fn new() -> Self {
        Self {
            handler: 0,
            mask: SignalFlag::empty(),
            flags: SignalActionFlags::empty(),
            restorer: 0,
        }
    }

    pub fn handler(&self) -> usize {
        self.handler
    }

    pub fn mask(&self) -> SignalFlag {
        self.mask
    }

    pub fn flags(&self) -> SignalActionFlags {
        self.flags
    }

    pub fn restorer(&self) -> usize {
        self.restorer
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SignalActionFlags: u32 {
        // const SA_NOCLDSTOP = 1;
        // const SA_NOCLDWAIT = 2;
        // const SA_SIGINFO = 4;
        const SA_RESTORER = 0x04_000_000;
        // const SA_ONSTACK = 0x08_000_000;
        // const SA_RESTART = 0x10_000_000;
        const SA_NODEFER  = 0x40_000_000;
        // const SA_RESETHAND = 0x80_000_000;
    }
}
