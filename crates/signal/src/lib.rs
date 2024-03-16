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

mod handlers;
mod receiver;

pub use handlers::DefaultHandler;
pub use handlers::SignalHandlers;
pub use receiver::SignalReceiver;

use defines::structs::KSignalSet;
use defines::structs::Signal;
use defines::trap_context::TrapContext;
use num_enum::TryFromPrimitive;

pub const SIG_ERR: usize = -1isize as usize;
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

pub struct SignalContext {
    pub old_mask: KSignalSet,
    pub old_trap_context: TrapContext,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(usize)]
#[allow(non_camel_case_types)]
pub enum SigprocmaskHow {
    /// 掩蔽传入的信号集，即新掩码是传入值和旧的并集
    SIG_BLOCK = 0,
    /// 取消掩蔽传入的信号集
    SIG_UNBLOCK = 1,
    /// 将掩码设置为传入的信号集，即直接赋值
    SIG_SETMASK = 2,
}
