use bitflags::bitflags;
use num_enum::TryFromPrimitive;

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
    pub mask: KSignalSet,
}

impl KSignalAction {
    pub const fn new() -> Self {
        Self {
            handler: 0,
            mask: KSignalSet::empty(),
            flags: SignalActionFlags::empty(),
            restorer: 0,
        }
    }

    pub fn handler(&self) -> usize {
        self.handler
    }

    pub fn mask(&self) -> KSignalSet {
        self.mask
    }

    pub fn flags(&self) -> SignalActionFlags {
        self.flags
    }

    pub fn restorer(&self) -> usize {
        self.restorer
    }
}

// TODO: 出于简单性，暂时只考虑标准信号，后续有需要实时信号再添加

bitflags! {
    /// 其实 posix 规定 64 位平台上应该有 1024bits
    /// [Why is sigset_t in glibc/musl 128 bytes large on 64-bit Linux?](https://unix.stackexchange.com/questions/399342/why-is-sigset-t-in-glibc-musl-128-bytes-large-on-64-bit-linux)
    ///
    /// 然而实践中比较混乱。比如理论应该区分 sigset_t(1024bits) 和 kernel_sigset_t(64bits?)，但 linux 内核中后者的名字是前者。
    ///
    /// 而在 syscall 边界上，linux 也是直接使用的 64bits 的
    #[derive(Clone, Copy, Debug)]
    pub struct KSignalSet: u64 {
        const SIGHUP    = 1 << (Signal::SIGHUP as u8);
        const SIGINT    = 1 << (Signal::SIGINT as u8);
        const SIGQUIT   = 1 << (Signal::SIGQUIT as u8);
        const SIGILL    = 1 << (Signal::SIGILL as u8);
        const SIGTRAP   = 1 << (Signal::SIGTRAP as u8);
        const SIGABRT   = 1 << (Signal::SIGABRT as u8);
        const SIGBUS    = 1 << (Signal::SIGBUS as u8);
        const SIGFPE    = 1 << (Signal::SIGFPE as u8);
        const SIGKILL   = 1 << (Signal::SIGKILL as u8);
        const SIGUSR1   = 1 << (Signal::SIGUSR1 as u8);
        const SIGSEGV   = 1 << (Signal::SIGSEGV as u8);
        const SIGUSR2   = 1 << (Signal::SIGUSR2 as u8);
        const SIGPIPE   = 1 << (Signal::SIGPIPE as u8);
        const SIGALRM   = 1 << (Signal::SIGALRM as u8);
        const SIGTERM   = 1 << (Signal::SIGTERM as u8);
        const SIGSTKFLT = 1 << (Signal::SIGSTKFLT as u8);
        const SIGCHLD   = 1 << (Signal::SIGCHLD as u8);
        const SIGCONT   = 1 << (Signal::SIGCONT as u8);
        const SIGSTOP   = 1 << (Signal::SIGSTOP as u8);
        const SIGTSTP   = 1 << (Signal::SIGTSTP as u8);
        const SIGTTIN   = 1 << (Signal::SIGTTIN as u8);
        const SIGTTOU   = 1 << (Signal::SIGTTOU as u8);
        const SIGURG    = 1 << (Signal::SIGURG as u8);
        const SIGXCPU   = 1 << (Signal::SIGXCPU as u8);
        const SIGXFSZ   = 1 << (Signal::SIGXFSZ as u8);
        const SIGVTALRM = 1 << (Signal::SIGVTALRM as u8);
        const SIGPROF   = 1 << (Signal::SIGPROF as u8);
        const SIGWINCH  = 1 << (Signal::SIGWINCH as u8);
        const SIGIO     = 1 << (Signal::SIGIO as u8);
        const SIGPWR    = 1 << (Signal::SIGPWR as u8);
        const SIGSYS    = 1 << (Signal::SIGSYS as u8);
    }
}

impl From<Signal> for KSignalSet {
    fn from(value: Signal) -> Self {
        Self::from_bits_truncate(value as u64)
    }
}

/// 注意，和 linux 不同，信号的编号从 0 开始而非从 1 开始。因此在一些系统调用上应当将传入的值减 1
#[derive(Debug, PartialEq, Eq, Clone, Copy, TryFromPrimitive)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub enum Signal {
    SIGHUP = 0,
    SIGINT = 1,
    SIGQUIT = 2,
    SIGILL = 3,
    SIGTRAP = 4,
    SIGABRT = 5,
    SIGBUS = 6,
    SIGFPE = 7,
    SIGKILL = 8,
    SIGUSR1 = 9,
    SIGSEGV = 10,
    SIGUSR2 = 11,
    SIGPIPE = 12,
    SIGALRM = 13,
    SIGTERM = 14,
    SIGSTKFLT = 15,
    SIGCHLD = 16,
    SIGCONT = 17,
    SIGSTOP = 18,
    SIGTSTP = 19,
    SIGTTIN = 20,
    SIGTTOU = 21,
    SIGURG = 22,
    SIGXCPU = 23,
    SIGXFSZ = 24,
    SIGVTALRM = 25,
    SIGPROF = 26,
    SIGWINCH = 27,
    SIGIO = 28,
    SIGPWR = 29,
    SIGSYS = 30,
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
