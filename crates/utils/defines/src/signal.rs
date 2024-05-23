use bitflags::bitflags;

/// 信号机制所需的 bitset 大小
pub const SIGSET_SIZE: usize = 64;
pub const SIGSET_SIZE_BYTES: usize = SIGSET_SIZE / 8;

/// 参考 musl 的 `k_sigaction`
#[repr(C)]
#[derive(Clone, Debug)]
pub struct KSignalAction {
    /// singal handler 的地址
    pub handler: usize,
    pub flags: SignalActionFlags,
    pub restorer: usize,
    pub mask: u64,
}

impl KSignalAction {
    pub const fn new() -> Self {
        Self {
            handler: 0,
            mask: 0,
            flags: SignalActionFlags::empty(),
            restorer: 0,
        }
    }
}

impl Default for KSignalAction {
    fn default() -> Self {
        Self::new()
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
        /// 一般而言。执行一个 signal handler 时，会屏蔽自己这个信号。
        ///
        /// 若指定以下这个 flag 则不会。sigaction 中的 mask 仍有效
        const SA_NODEFER  = 0x40_000_000;
        // const SA_RESETHAND = 0x80_000_000;
    }
}

pub const SIGHUP: u8 = 1;
pub const SIGINT: u8 = 2;
pub const SIGQUIT: u8 = 3;
pub const SIGILL: u8 = 4;
pub const SIGTRAP: u8 = 5;
pub const SIGABRT: u8 = 6;
pub const SIGBUS: u8 = 7;
pub const SIGFPE: u8 = 8;
pub const SIGKILL: u8 = 9;
pub const SIGUSR1: u8 = 10;
pub const SIGSEGV: u8 = 11;
pub const SIGUSR2: u8 = 12;
pub const SIGPIPE: u8 = 13;
pub const SIGALRM: u8 = 14;
pub const SIGTERM: u8 = 15;
pub const SIGSTKFLT: u8 = 16;
pub const SIGCHLD: u8 = 17;
pub const SIGCONT: u8 = 18;
pub const SIGSTOP: u8 = 19;
pub const SIGTSTP: u8 = 20;
pub const SIGTTIN: u8 = 21;
pub const SIGTTOU: u8 = 22;
pub const SIGURG: u8 = 23;
pub const SIGXCPU: u8 = 24;
pub const SIGXFSZ: u8 = 25;
pub const SIGVTALRM: u8 = 26;
pub const SIGPROF: u8 = 27;
pub const SIGWINCH: u8 = 28;
pub const SIGIO: u8 = 29;
pub const SIGPWR: u8 = 30;
pub const SIGSYS: u8 = 31;
