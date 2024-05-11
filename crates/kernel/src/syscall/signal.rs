use defines::{
    error::{errno, KResult},
    signal::{KSignalAction, KSignalSet, Signal, SignalActionFlags, SIGSET_SIZE_BYTES},
};

use crate::{
    hart::local_hart,
    memory::UserCheck,
    process::exit_process,
    signal::{SignalContext, SigprocmaskHow},
};

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
    act: UserCheck<KSignalAction>,
    old_act: UserCheck<KSignalAction>,
) -> KResult {
    let Ok(signal) = Signal::try_from(signum as u8) else {
        warn!("use unsupported signal {signum}");
        return Err(errno::EINVAL);
    };
    debug!("read/write {signal:?}'s action");
    if signal == Signal::SIGKILL || signal == Signal::SIGSTOP {
        return Err(errno::EINVAL);
    }
    if !old_act.is_null() {
        let old_act_ptr = unsafe { old_act.check_ptr_mut()? };

        local_hart().curr_process().lock_inner_with(|inner| {
            old_act_ptr.write(inner.signal_handlers.action(signal).clone());
        });
    }

    if !act.is_null() {
        let act = act.check_ptr()?.read();
        if !act.flags.contains(SignalActionFlags::SA_RESTORER) {
            // `SA_RESTORER` 表示传入的 `restore` 字段是有用的
            // 一般而言这个字段由 libc 填写，用于 signal handler 执行结束之后调用 `sys_sigreturn`
            // 如果没有填写，则 os 需要自己手动做一个 trampoline
            todo!("[low] sig trampoline does not impl")
        }
        local_hart().curr_process().lock_inner_with(|inner| {
            inner.signal_handlers.action_mut(signal).clone_from(&act);
        });
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
    set: UserCheck<KSignalSet>,
    old_set: UserCheck<KSignalSet>,
    set_size: usize,
) -> KResult {
    if set_size > SIGSET_SIZE_BYTES {
        return Err(errno::EINVAL);
    }

    if !old_set.is_null() {
        let old_set_ptr = unsafe { old_set.check_ptr_mut()? };

        local_hart().curr_thread().lock_inner_with(|inner| {
            old_set_ptr.write(inner.signal_mask);
        });
    }

    let Ok(how) = SigprocmaskHow::try_from(how) else {
        return Err(errno::EINVAL);
    };

    if !set.is_null() {
        debug!("write signal mask with how = {how:?}");
        let set_ptr = set.check_ptr()?.read();
        local_hart()
            .curr_thread()
            .lock_inner_with(|inner| match how {
                SigprocmaskHow::SIG_BLOCK => inner.signal_mask.insert(set_ptr),
                SigprocmaskHow::SIG_UNBLOCK => inner.signal_mask.remove(set_ptr),
                SigprocmaskHow::SIG_SETMASK => inner.signal_mask = set_ptr,
            });
    }

    Ok(0)
}

pub fn sys_rt_sigreturn() -> KResult {
    debug!("sigreturn called");
    let thread = local_hart().curr_thread();
    let sp = thread.lock_inner_with(|inner| inner.trap_context.sp());
    let Ok(old_ctx) = UserCheck::new(sp as *mut SignalContext).check_ptr() else {
        // TODO:[blocked] 这里其实可以试着补救
        exit_process(&thread.process, -10);
        return Err(errno::BREAK);
    };
    let old_ctx = old_ctx.read();

    thread.lock_inner_with(|inner| {
        inner.signal_mask = old_ctx.old_mask;
        inner.trap_context = old_ctx.old_trap_context;
    });

    Ok(0)
}
