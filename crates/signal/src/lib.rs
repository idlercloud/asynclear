//! 参考：<https://man7.org/linux/man-pages/man7/signal.7.html>
//!
//! signal action 是属于进程的，而线程可以有各自的掩码和待处理信号
//!
//! `fork` 会继承父进程的 signal action 和线程的掩码，但是线程的待处理信号会置空。
//!
//! 而 `execve` 会将 signal action 置为默认值（可能与 linux 不同），但是线程掩码和待处理信号保留
//!
//! 信号处理的过程：
//!
//! 1. 将该信号从线程的待处理信号集中移除
//! 2. ？

// TODO: 参考 <https://man7.org/linux/man-pages/man7/signal.7.html> 和 <https://man7.org/linux/man-pages/man2/rt_sigaction.2.html> 完善 signal 相关文档

#![no_std]

mod action;
mod handlers;
mod receiver;

pub use action::{KSignalAction, SignalAction, SignalActionFlags};
use defines::trap_context::TrapContext;
pub use handlers::DefaultHandler;
pub use handlers::SignalHandlers;
pub use receiver::SignalReceiver;

pub const SIG_ERR: usize = -1isize as usize;
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

use bitflags::bitflags;
use num_enum::TryFromPrimitive;

pub struct SignalContext {
    pub old_mask: SignalFlag,
    pub old_trap_context: TrapContext,
}

// 这里只考虑了 64 位！

/// `SingalSet` 也即 `sigset_t`，总大小 1024 bits。
///
/// 实际上可能只会用到 32 或 64 bits。
///
/// 详见 [Why is sigset_t in glibc/musl 128 bytes large on 64-bit Linux?](https://unix.stackexchange.com/questions/399342/why-is-sigset-t-in-glibc-musl-128-bytes-large-on-64-bit-linux)
#[derive(Clone, Debug)]
#[repr(C)]
pub struct SignalSet {
    flags: [SignalFlag; 16],
}

impl SignalSet {
    pub const fn empty() -> Self {
        Self {
            flags: [SignalFlag::empty(); 16],
        }
    }

    pub fn with_flag(flag: SignalFlag) -> Self {
        let mut flags = [SignalFlag::empty(); 16];
        flags[0] = flag;
        Self { flags }
    }

    pub fn flag(&self) -> SignalFlag {
        self.flags[0]
    }

    pub fn flag_mut(&mut self) -> &mut SignalFlag {
        &mut self.flags[0]
    }
}

// TODO: 出于简单性，暂时只考虑标准信号，后续有需要实时信号再添加

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SignalFlag: u64 {
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

impl From<Signal> for SignalFlag {
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

impl Signal {
    #[allow(clippy::enum_glob_use)]
    pub fn default_handler(self) -> DefaultHandler {
        use DefaultHandler::*;
        use Signal::*;
        match self {
            SIGABRT | SIGBUS | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU
            | SIGXFSZ => CoreDump,
            SIGCHLD | SIGURG | SIGWINCH => Ignore,
            SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => Stop,
            SIGCONT => Continue,
            _ => Terminate,
        }
    }
}

#[derive(Debug, TryFromPrimitive)]
#[repr(usize)]
#[allow(clippy::upper_case_acronyms)]
pub enum SigprocmaskHow {
    /// 掩蔽传入的信号集，即新掩码是传入值和旧的并集
    SIGBLOCK = 0,
    /// 取消掩蔽传入的信号集
    SIGUNBLOCK = 1,
    /// 将掩码设置为传入的信号集，即直接赋值
    SIGSETMASK = 2,
}
