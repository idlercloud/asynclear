use defines::{
    error::{errno, KResult},
    signal::{KSignalAction, SignalActionFlags, SIGSET_SIZE_BYTES},
};

use crate::{
    hart::local_hart,
    memory::UserCheck,
    process::{exit_process, PROCESS_MANAGER},
    signal::{KSignalSet, SigProcMaskHow, Signal, SignalContext},
};

/// 设置当前**进程**在收到特定信号时的行为
///
/// 参数：
/// - `signum` 指定信号，可以是除了 `SIGKILL` 和 `SIGSTOP` 之外的任意有效信号。见 [`Signal`]
/// - `new_act` 如果非 NULL，则安装 `new_act` 指向的新操作
/// - `old_act` 如果非 NULL，则将旧操作写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `new_act` 或者 `old_act` 指向非法地址
/// - `EINVAL` 如果 signum 不是除了 `SIGKILL` 和 `SIGSTOP` 之外的有效信号
pub fn sys_rt_sigaction(
    signum: usize,
    new_act: Option<UserCheck<KSignalAction>>,
    old_act: Option<UserCheck<KSignalAction>>,
) -> KResult {
    let Some(signal) = Signal::from_user(signum as u8) else {
        warn!("use unsupported signal {signum}");
        return Err(errno::EINVAL);
    };
    debug!("read/write {signal:?}'s action");
    if signal == Signal::SIGKILL || signal == Signal::SIGSTOP {
        return Err(errno::EINVAL);
    }
    if let Some(old_act) = old_act {
        let old_act_ptr = unsafe { old_act.check_ptr_mut()? };

        local_hart().curr_process().lock_inner_with(|inner| {
            old_act_ptr.write(inner.signal_handlers.action(signal).clone());
        });
    }

    if let Some(new_act) = new_act {
        let act = new_act.check_ptr()?.read();
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
/// - `new_set` 是用户传入的，希望设置的掩码集，具体如何使用取决于 `how`
/// - `old_set` 如果非 NULL，则将旧的信号掩码写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `new_set` 或 `old_set` 指向非法地址
/// - `EINVAL` 如果 `how` 参数非法或者内核不支持 `set_size`
pub fn sys_rt_sigprocmask(
    how: usize,
    new_set: Option<UserCheck<u64>>,
    old_set: Option<UserCheck<u64>>,
    set_size: usize,
) -> KResult {
    if set_size > SIGSET_SIZE_BYTES {
        return Err(errno::EINVAL);
    }

    if let Some(old_set) = old_set {
        let old_set_ptr = unsafe { old_set.check_ptr_mut()? };

        local_hart().curr_thread().lock_inner_with(|inner| {
            old_set_ptr.write(inner.signal_mask.to_user());
        });
    }

    let how = SigProcMaskHow::from_user(how).ok_or(errno::EINVAL)?;

    if let Some(new_set) = new_set {
        debug!("write signal mask with how = {how:?}");
        let new_set = KSignalSet::from_user(new_set.check_ptr()?.read());
        local_hart()
            .curr_thread()
            .lock_inner_with(|inner| match how {
                SigProcMaskHow::Block => inner.signal_mask.insert(new_set),
                SigProcMaskHow::Unblock => inner.signal_mask.remove(new_set),
                SigProcMaskHow::SetMask => inner.signal_mask = new_set,
            });
    }

    Ok(0)
}

pub fn sys_rt_sigreturn() -> KResult {
    let thread = local_hart().curr_thread();
    let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };
    let Ok(old_ctx) = UserCheck::new(trap_context.sp() as *mut SignalContext)
        .ok_or(errno::EINVAL)?
        .check_ptr()
    else {
        exit_process(&thread.process, -10);
        return Err(errno::BREAK);
    };
    let old_ctx = old_ctx.read();

    thread.lock_inner_with(|inner| inner.signal_mask = old_ctx.old_mask);
    *trap_context = old_ctx.old_trap_context;

    Ok(trap_context.a0())
}

pub fn sys_kill(pid: isize, signum: usize) -> KResult {
    let signal = if signum != 0 {
        Some(
            Signal::from_user(u8::try_from(signum).map_err(|e| {
                warn!("convert signum {signum} to u8 failed: {e}");
                errno::EINVAL
            })?)
            .ok_or(errno::EINVAL)?,
        )
    } else {
        None
    };
    // TODO: [low] 发送信号需要权限检查
    if pid > 0 {
        let Some(process) = PROCESS_MANAGER.get(pid as usize) else {
            return Err(errno::ESRCH);
        };
        if let Some(signal) = signal {
            debug!("send signal {signal:?} to pid {pid}");
            process.lock_inner_with(|inner| inner.receive_signal(signal));
        }
    } else if pid == 0 {
        todo!("[blocked] process group. send signal to process group");
    } else if pid == -1 {
        let Some(signal) = signal else {
            return Ok(0);
        };
        debug!("send signal {signal:?} to all processes");
        let all_processes = PROCESS_MANAGER.lock_all();
        for (&pid, process) in all_processes.iter() {
            if pid == 1 {
                continue;
            }
            process.lock_inner_with(|inner| inner.receive_signal(signal));
        }
    } else if pid < -1 {
        todo!("[blocked] process group. send signal to process group");
    }
    Ok(0)
}
