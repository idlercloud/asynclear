use defines::{
    config::SIGSET_SIZE_BYTES,
    error::{errno, Result},
};
use signal::{Signal, SignalAction, SignalSet, SigprocmaskHow};
use user_check::{UserCheck, UserCheckMut};

use crate::hart::local_hart;

/// 设置当前**进程**在收到特定信号时的行为
///
/// 参数：
/// - `signum` 指定信号，可以是除了 `SIGKILL` 和 `SIGSTOP` 之外的任意有效信号。见 [`Signal`]
/// - `act` 如果非 NULL，则安装 `act` 指向的新操作
/// - `old_act` 如果非 NULL，则将旧操作写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `act` 或者 `old_act` 指向非法地址
/// - `EINVAL` 如果 signum 不是除了 `SIGKILL` 和 `SIGSTOP` 之外的有效信号
pub fn sys_rt_sigaction(
    signum: usize,
    act: *const SignalAction,
    old_act: *mut SignalAction,
) -> Result {
    let Ok(signal) = Signal::try_from(signum as u8) else {
        warn!("use unsupported signal {signum}");
        return Err(errno::EINVAL);
    };
    debug!("read/write {signal:?}'s action");
    if signal == Signal::SIGKILL || signal == Signal::SIGSTOP {
        return Err(errno::EINVAL);
    }
    if !old_act.is_null() {
        trace!("read old_act into {old_act:p}");
        let mut old_act_ptr = UserCheckMut::new(old_act).check_ptr_mut()?;

        unsafe {
            (*local_hart()).curr_process().lock_inner_with(|inner| {
                old_act_ptr.clone_from(inner.signal_handlers.action(signal));
            });
        }
    }

    if !act.is_null() {
        trace!("write sigaction from {act:p}");
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

// TODO: 其实 `sys_rt_sigprocmask` 应该只对传统的单线程进程生效，多线程应该使用 `pthread_sigmask`
/// 用于获取或更改**线程**的信号掩码
///
/// 参数：
/// - `how` 指定该调用的行为，具体见 [`SigprocmaskHow`]
/// - `set` 是用户传入的，希望设置的掩码集，具体如何使用取决于 `how`
/// - `old_set` 如果非 NULL，则将旧的信号掩码写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `set` 或 `old_set` 指向非法地址
/// - `EINVAL` 如果 `how` 参数非法或者内核不支持 `set_size`
pub fn sys_rt_sigprocmask(
    how: usize,
    set: *const SignalSet,
    old_set: *mut SignalSet,
    set_size: usize,
) -> Result {
    if set_size > SIGSET_SIZE_BYTES {
        return Err(errno::EINVAL);
    }

    if !old_set.is_null() {
        trace!("read old_set into {old_set:p}");
        let mut old_set_ptr = UserCheckMut::new(old_set).check_ptr_mut()?;

        unsafe {
            (*local_hart()).curr_thread().lock_inner_with(|inner| {
                old_set_ptr.flag_mut().clone_from(&inner.signal_mask);
            });
        }
    }

    let Ok(how) = SigprocmaskHow::try_from(how) else {
        return Err(errno::EINVAL);
    };

    if !set.is_null() {
        debug!("write signal mask from {set:p} with how = {how:?}");
        let set_ptr = UserCheck::new(set).check_ptr()?;
        let flag = set_ptr.flag();
        unsafe {
            (*local_hart())
                .curr_thread()
                .lock_inner_with(|inner| match how {
                    SigprocmaskHow::SIGBLOCK => inner.signal_mask.insert(flag),
                    SigprocmaskHow::SIGUNBLOCK => inner.signal_mask.remove(flag),
                    SigprocmaskHow::SIGSETMASK => inner.signal_mask = flag,
                });
        }
    }

    Ok(0)
}
