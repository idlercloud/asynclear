//! 参考：<https://man7.org/linux/man-pages/man7/signal.7.html>
//!
//! signal action 是属于进程的，而线程可以有各自的掩码和待处理信号
//!
//! `fork` 会继承父进程的 signal action 和线程的掩码，但是线程的待处理信号会置空。
//!
//! 而 `execve` 会将 signal action 置为默认值（可能与 linux 不同），但是线程掩码和待处理信号保留

mod handlers;

use bitflags::bitflags;
use defines::signal::KSignalAction;
use extend::ext;
pub use handlers::{DefaultHandler, SignalHandlers};

use crate::trap::TrapContext;

pub const SIG_ERR: usize = usize::MAX;
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

pub struct SignalContext {
    pub old_mask: KSignalSet,
    pub old_trap_context: TrapContext,
}

#[derive(Debug)]
pub enum SigProcMaskHow {
    /// 掩蔽传入的信号集，即新掩码是传入值和旧的并集
    Block,
    /// 取消掩蔽传入的信号集
    Unblock,
    /// 将掩码设置为传入的信号集，即直接赋值
    SetMask,
}

impl SigProcMaskHow {
    pub fn from_user(how: usize) -> Option<Self> {
        match how {
            0 => Some(Self::Block),
            1 => Some(Self::Unblock),
            2 => Some(Self::SetMask),
            _ => None,
        }
    }
}

// TODO: 出于简单性，暂时只考虑标准信号，后续有需要实时信号再添加

bitflags! {
    /// 其实 posix 规定 64 位平台上应该有 1024bits。[Why is sigset_t in glibc/musl 128 bytes large on 64-bit Linux?](https://unix.stackexchange.com/questions/399342/why-is-sigset-t-in-glibc-musl-128-bytes-large-on-64-bit-linux)
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

impl KSignalSet {
    pub fn first_pending(self) -> Option<Signal> {
        Signal::from_user(self.bits().trailing_zeros() as u8 + 1)
    }
}

impl From<Signal> for KSignalSet {
    fn from(value: Signal) -> Self {
        Self::from_bits_truncate(1 << (value as u8))
    }
}

/// 注意，和 linux 不同，信号的编号从 0 开始而非从 1
/// 开始。因此在一些系统调用上应当将传入的值减 1，传出的值加 1
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub enum Signal {
    SIGHUP = defines::signal::SIGHUP - 1,
    SIGINT = defines::signal::SIGINT - 1,
    SIGQUIT = defines::signal::SIGQUIT - 1,
    SIGILL = defines::signal::SIGILL - 1,
    SIGTRAP = defines::signal::SIGTRAP - 1,
    SIGABRT = defines::signal::SIGABRT - 1,
    SIGBUS = defines::signal::SIGBUS - 1,
    SIGFPE = defines::signal::SIGFPE - 1,
    SIGKILL = defines::signal::SIGKILL - 1,
    SIGUSR1 = defines::signal::SIGUSR1 - 1,
    SIGSEGV = defines::signal::SIGSEGV - 1,
    SIGUSR2 = defines::signal::SIGUSR2 - 1,
    SIGPIPE = defines::signal::SIGPIPE - 1,
    SIGALRM = defines::signal::SIGALRM - 1,
    SIGTERM = defines::signal::SIGTERM - 1,
    SIGSTKFLT = defines::signal::SIGSTKFLT - 1,
    SIGCHLD = defines::signal::SIGCHLD - 1,
    SIGCONT = defines::signal::SIGCONT - 1,
    SIGSTOP = defines::signal::SIGSTOP - 1,
    SIGTSTP = defines::signal::SIGTSTP - 1,
    SIGTTIN = defines::signal::SIGTTIN - 1,
    SIGTTOU = defines::signal::SIGTTOU - 1,
    SIGURG = defines::signal::SIGURG - 1,
    SIGXCPU = defines::signal::SIGXCPU - 1,
    SIGXFSZ = defines::signal::SIGXFSZ - 1,
    SIGVTALRM = defines::signal::SIGVTALRM - 1,
    SIGPROF = defines::signal::SIGPROF - 1,
    SIGWINCH = defines::signal::SIGWINCH - 1,
    SIGIO = defines::signal::SIGIO - 1,
    SIGPWR = defines::signal::SIGPWR - 1,
    SIGSYS = defines::signal::SIGSYS - 1,
}

impl Signal {
    pub fn from_user(signum: u8) -> Option<Signal> {
        match signum {
            defines::signal::SIGHUP => Some(Signal::SIGHUP),
            defines::signal::SIGINT => Some(Signal::SIGINT),
            defines::signal::SIGQUIT => Some(Signal::SIGQUIT),
            defines::signal::SIGILL => Some(Signal::SIGILL),
            defines::signal::SIGTRAP => Some(Signal::SIGTRAP),
            defines::signal::SIGABRT => Some(Signal::SIGABRT),
            defines::signal::SIGBUS => Some(Signal::SIGBUS),
            defines::signal::SIGFPE => Some(Signal::SIGFPE),
            defines::signal::SIGKILL => Some(Signal::SIGKILL),
            defines::signal::SIGUSR1 => Some(Signal::SIGUSR1),
            defines::signal::SIGSEGV => Some(Signal::SIGSEGV),
            defines::signal::SIGUSR2 => Some(Signal::SIGUSR2),
            defines::signal::SIGPIPE => Some(Signal::SIGPIPE),
            defines::signal::SIGALRM => Some(Signal::SIGALRM),
            defines::signal::SIGTERM => Some(Signal::SIGTERM),
            defines::signal::SIGSTKFLT => Some(Signal::SIGSTKFLT),
            defines::signal::SIGCHLD => Some(Signal::SIGCHLD),
            defines::signal::SIGCONT => Some(Signal::SIGCONT),
            defines::signal::SIGSTOP => Some(Signal::SIGSTOP),
            defines::signal::SIGTSTP => Some(Signal::SIGTSTP),
            defines::signal::SIGTTIN => Some(Signal::SIGTTIN),
            defines::signal::SIGTTOU => Some(Signal::SIGTTOU),
            defines::signal::SIGURG => Some(Signal::SIGURG),
            defines::signal::SIGXCPU => Some(Signal::SIGXCPU),
            defines::signal::SIGXFSZ => Some(Signal::SIGXFSZ),
            defines::signal::SIGVTALRM => Some(Signal::SIGVTALRM),
            defines::signal::SIGPROF => Some(Signal::SIGPROF),
            defines::signal::SIGWINCH => Some(Signal::SIGWINCH),
            defines::signal::SIGIO => Some(Signal::SIGIO),
            defines::signal::SIGPWR => Some(Signal::SIGPWR),
            defines::signal::SIGSYS => Some(Signal::SIGSYS),
            _ => None,
        }
    }

    pub fn to_user(self) -> u8 {
        self as u8 + 1
    }
}

#[ext]
pub impl KSignalAction {
    fn kmask(&self) -> KSignalSet {
        KSignalSet::from_bits_truncate(self.mask >> 1)
    }
}
