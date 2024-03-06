use defines::error::{errno, Result};
use signal::{Signal, SignalAction};
use user_check::{UserCheck, UserCheckMut};

use crate::hart::local_hart;

/// 设置当前*进程*在收到特定信号时的行为
///
/// 参数：
/// - `signum` 指定信号，可以是除了 `SIGKILL` 和 `SIGSTOP` 之外的任意有效信号。见 [`Signal`]
/// - `act` 如果非 NULL，则安装 `act` 指向的新操作
/// - `old_act` 如果非 NULL，则将旧操作写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `act` 或者 `old_act` 指向非法地址
/// - `EINVAL` 如果 signum 不是除了 `SIGKILL` 和 `SIGSTOP` 之外的有效信号
///
/// [`Signal`]: signal::Signal
pub fn sys_rt_sigaction(
    signum: usize,
    act: *const SignalAction,
    old_act: *mut SignalAction,
) -> Result {
    let Ok(signal) = Signal::try_from(signum as u8) else {
        return Err(errno::EINVAL);
    };
    if signal == Signal::SIGKILL || signal == Signal::SIGSTOP {
        return Err(errno::EINVAL);
    }
    if !old_act.is_null() {
        let mut old_act_ptr = UserCheckMut::new(old_act).check_ptr_mut()?;

        unsafe {
            (*local_hart()).curr_process().lock_inner_with(|inner| {
                old_act_ptr.clone_from(inner.signal_handlers.action(signal));
            });
        }
    }
    if !act.is_null() {
        let act_ptr = UserCheck::new(act).check_ptr()?;
        unsafe {
            (*local_hart()).curr_process().lock_inner_with(|inner| {
                inner
                    .signal_handlers
                    .action_mut(signal)
                    .clone_from(&act_ptr);
            });
        }
    }

    Ok(0)
}
