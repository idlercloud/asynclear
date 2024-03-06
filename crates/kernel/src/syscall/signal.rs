use defines::error::Result;
use signal::SignalAction;

/// 设置当前*进程*在收到特定信号时的行为
///
/// 参数：
/// - `signum` 指定信号，可以是除了 `SIGKILL` 和 `SIGSTOP` 之外的任意有效信号。见 [`SignalFlag`]
/// - `act` 如果非 NULL，则安装 `act` 指向的新操作
/// - `old_act` 如果非 NULL，则将旧操作写入 `old_act` 中
///
/// 错误：
/// - `EFAULT` 如果 `act` 或者 `old_act` 指向非法地址
/// - `EINVAL` 如果 signum 不是除了 `SIGKILL` 和 `SIGSTOP` 之外的有效信号
///
/// [`SignalFlag`]: signal::SignalFlag
pub fn sys_rt_sigaction(
    signum: usize,
    act: *const SignalAction,
    old_act: *mut SignalAction,
) -> Result {
    todo!()
}
