use super::SignalSet;
use bitflags::bitflags;

#[allow(unused)]
const SIG_ERR: usize = -1_isize as usize;
#[allow(unused)]
const SIG_DFL: usize = 0;
#[allow(unused)]
const SIG_IGN: usize = 1;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct SignalAction {
    /// singal handler 的地址
    handler: usize,
    mask: SignalSet,
    flags: SignalActionFlags,
    restorer: usize,
}

impl SignalAction {
    pub const fn new() -> Self {
        Self {
            handler: 0,
            flags: SignalActionFlags::empty(),
            restorer: 0,
            mask: SignalSet::empty(),
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SignalActionFlags: u32 {
        // const SA_NOCLDSTOP = 1;
        // const SA_NOCLDWAIT = 2;
        // const SA_SIGINFO = 4;
        // const SA_ONSTACK = 0x08000000;
        // const SA_RESTART = 0x10000000;
        // const SA_NODEFER = 0x40000000;
        // const SA_RESETHAND = 0x80000000;
        const SA_RESTORER = 0x04000000;
    }
}
