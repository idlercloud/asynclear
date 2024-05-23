//! Process management syscalls

use alloc::vec::Vec;
use core::num::NonZeroUsize;

use atomic::Ordering;
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::{CloneFlags, UtsName, WaitFlags},
};
use event_listener::listener;
use triomphe::Arc;

use crate::{
    executor,
    hart::local_hart,
    memory::UserCheck,
    process::{exit_process, INITPROC},
    signal::Signal,
    thread::BlockingFuture,
};

/// 退出当前线程，结束用户线程循环。
pub fn sys_exit(exit_code: i32) -> KResult {
    local_hart()
        .curr_thread()
        .exit_code
        .store((exit_code & 0xff) as i8, Ordering::SeqCst);
    Err(errno::BREAK)
}

/// 退出进程，退出所有线程
pub fn sys_exit_group(exit_code: i32) -> KResult {
    exit_process(&local_hart().curr_process(), (exit_code & 0xff) as i8);
    Err(errno::BREAK)
}

/// 挂起当前任务，让出 CPU，永不失败
pub async fn sys_sched_yield() -> KResult {
    executor::yield_now().await;
    Ok(0)
}

/// 返回当前进程 id，永不失败
pub fn sys_getpid() -> KResult {
    let pid = local_hart().curr_process().pid() as isize;
    Ok(pid)
}

/// 返回当前进程的父进程的 id，永不失败
pub fn sys_getppid() -> KResult {
    // INITPROC(pid=1) 没有父进程，返回 0
    let ppid = local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.parent.as_ref().map_or(0, |p| p.pid()) as isize);
    Ok(ppid)
}

/// 创建子任务，通过 flags 进行精确控制。父进程返回子进程 pid，子进程返回 0。
///
/// TODO: 完善 `sys_clone()` 及文档
///
/// 参数：
/// - `flags` 低八位 `exit_signal`，高位指定 clone 的方式。具体参看
///   [`CloneFlags`]
/// - `user_stack` 指定用户栈的
pub fn sys_clone(
    flags: usize,
    user_stack: usize,
    _ptid: usize,
    _tls: usize,
    _ctid: usize,
) -> KResult {
    let Ok(flags) = u32::try_from(flags) else {
        error!("flags exceeds u32: {flags:#b}");
        return Err(errno::UNSUPPORTED);
    };

    // 参考 https://man7.org/linux/man-pages/man2/clone.2.html，低 8 位是 exit_signal，其余是 clone flags
    let Some(clone_flags) = CloneFlags::from_bits(flags & !0xff) else {
        error!("undefined CloneFlags: {:#b}", flags & !0xff);
        return Err(errno::UNSUPPORTED);
    };
    if clone_flags.contains(CloneFlags::CLONE_THREAD) {
        // 创建线程的情况。这些 flag 应该同时设置
        assert!(clone_flags.contains(
            CloneFlags::CLONE_VM
                | CloneFlags::CLONE_FS
                | CloneFlags::CLONE_FILES
                | CloneFlags::CLONE_SIGHAND
                | CloneFlags::CLONE_THREAD
                | CloneFlags::CLONE_SYSVSEM
                | CloneFlags::CLONE_SETTLS
                | CloneFlags::CLONE_PARENT_SETTID
                | CloneFlags::CLONE_CHILD_CLEARTID
        ));

        // 创建线程时不该有 `exit_signal`
        if flags as u8 != 0 {
            warn!(
                "create thread not allowed to set exit_signal: {:#b}",
                flags as u8
            );
            return Err(errno::EINVAL);
        }
        todo!("[mid] support create thread");
    } else {
        // 创建进程的情况。这些 flag 都不应该设置
        assert!(!clone_flags.intersects(
            CloneFlags::CLONE_VM
                | CloneFlags::CLONE_FS
                | CloneFlags::CLONE_FILES
                | CloneFlags::CLONE_SIGHAND
                | CloneFlags::CLONE_THREAD
                | CloneFlags::CLONE_SYSVSEM
                | CloneFlags::CLONE_SETTLS
                | CloneFlags::CLONE_PARENT_SETTID
                | CloneFlags::CLONE_CHILD_CLEARTID
        ));
        let signum = flags as u8;
        let mut exit_signal = None;
        if signum != 0 {
            let Some(signal) = Signal::from_user(flags as u8) else {
                error!("undefined signal: {:#b}", flags as u8);
                return Err(errno::UNSUPPORTED);
            };
            if signal != Signal::SIGCHLD {
                todo!("[low] unsupported signal for exit_signal: {signal:?}");
            }
            debug!("exit signal is {signal:?}");
            exit_signal = Some(signal);
        }
        let user_stack = NonZeroUsize::new(user_stack);
        let new_process = local_hart()
            .curr_thread()
            .process
            .fork(user_stack, exit_signal);
        Ok(new_process.pid() as isize)
    }
}

/// 将当前进程的地址空间清空并加载一个特定的可执行文件，
/// 返回用户态后开始它的执行。返回参数个数
///
/// 参数：
/// - `pathname` 给出了要加载的可执行文件的名字，必须以 `\0` 结尾
/// - `argv` 给出了参数列表。其最后一个元素必须是 0
/// - `envp` 给出环境变量列表，其最后一个元素必须是 0
pub fn sys_execve(
    pathname: UserCheck<u8>,
    argv: UserCheck<usize>,
    envp: UserCheck<usize>,
) -> KResult {
    let pathname = pathname.check_cstr()?;
    debug!("pathname: {}", &*pathname);
    // 收集参数列表
    let collect_cstrs = |mut ptr_vec: UserCheck<usize>| -> KResult<Vec<CompactString>> {
        let mut v = Vec::new();
        loop {
            // TODO: [low] 这里其实重复检查了，或许可以优化。要注意对齐要求
            let arg_str_ptr = ptr_vec.check_ptr()?.read();
            if arg_str_ptr == 0 {
                break;
            }
            let arg_str = UserCheck::new(arg_str_ptr as *mut u8).check_cstr()?;
            v.push(CompactString::from(&*arg_str));
            ptr_vec = ptr_vec.add(1);
        }
        Ok(v)
    };
    let args = collect_cstrs(argv)?;
    let envs = if envp.is_null() {
        Vec::new()
    } else {
        collect_cstrs(envp)?
    };

    // 执行新进程

    let argc = args.len();
    local_hart()
        .curr_process()
        .exec(CompactString::from(&*pathname), args, envs)?;
    Ok(argc as isize)
}

/// 挂起本线程，等待子进程改变状态（终止、或信号处理）。默认而言，
/// 会阻塞式等待子进程终止。
///
/// 若成功，返回子进程 pid，若 `options` 指定了 WNOHANG
/// 且子线程存在但状态为改变，则返回 0
///
/// 参数：
/// - `pid` 要等待的 pid
///     - `pid` < -1，则等待一个 pgid 为 `pid` 绝对值的子进程，目前不支持
///     - `pid` == -1，则等待任意一个子进程
///     - `pid` == 0，则等待一个 pgid 与调用进程**调用时**的 pgid
///       相同的子进程，目前不支持
///     - `pid` > 0，则等待指定 `pid` 的子进程
/// - `wstatus: *mut i32` 指向一个
///   int，若非空则用于表示某些状态，目前而言似乎仅需往里写入子进程的 exit code
/// - `options` 控制等待方式，详细查看 [`WaitFlags`]，目前只支持 `WNOHANG`
/// - `rusgae` 用于统计子进程资源使用情况，目前不支持
pub async fn sys_wait4(
    pid: isize,
    wstatus: UserCheck<i32>,
    options: usize,
    rusage: usize,
) -> KResult {
    assert!(
        pid == -1 || pid > 0,
        "pid < -1 和 pid == 0，也就是等待进程组，目前还不支持"
    );
    assert_eq!(rusage, 0, "目前 rusage 尚不支持，所以必须为 nullptr");
    let options = WaitFlags::from_bits(options as u32).ok_or(errno::EINVAL)?;
    if options.contains(WaitFlags::WIMTRACED) || options.contains(WaitFlags::WCONTINUED) {
        error!("暂时仅支持 WNOHANG");
        return Err(errno::UNSUPPORTED);
    }

    // 尝试找到一个符合条件，且已经是僵尸的子进程
    let process = Arc::clone(&*local_hart().curr_process_arc());
    loop {
        listener!(process.wait4_event => listener);
        {
            // 用块是因为 rust 目前不够聪明。
            // inner 是个 Guard，不 Send，因此不能包含在 future 中
            // 但在同一块作用域中，即使 drop inner，也依然会导致 future 不 send，非常麻烦
            let mut inner = process.lock_inner();
            let mut has_proper_child = false;
            let mut child_index = None;
            for (index, child) in inner.children.iter().enumerate() {
                if pid == -1 || child.pid() == pid as usize {
                    has_proper_child = true;
                    if child.is_zombie() {
                        child_index = Some(index);
                    }
                }
            }

            if !has_proper_child {
                return Err(errno::ECHILD);
            }

            if let Some(index) = child_index {
                let child = inner.children.remove(index);
                drop(inner);
                let found_pid = child.pid();
                let exit_code = child.exit_code().expect("Thread should be zombie");
                if !wstatus.is_null() {
                    let wstatus = unsafe { wstatus.check_ptr_mut()? };
                    // *wstatus 的构成，可能要参考 WEXITSTATUS 那几个宏
                    wstatus.write((exit_code as u8 as i32) << 8);
                }
                return Ok(found_pid as isize);
            }

            // 否则视 `options` 而定
            if options.contains(WaitFlags::WNOHANG) {
                return Ok(0);
            }
        };

        trace!("no proper child exited");
        BlockingFuture::new(listener).await;
    }
}

pub fn sys_setpriority(_prio: isize) -> KResult {
    todo!("[low] sys_setpriority")
}

/// 设置线程控制块中 `clear_child_tid` 的值为 `tidptr`。总是返回调用者线程的 tid。
///
/// 参数：
/// - `tidptr`。
///   - 注意该参数此时是不进行检查的，因此该系统调用永不失败。
///   - 在 linux 手册中，`tidptr` 的类型是 int*。
///   - 这里设置为 i32，是参考 libc crate 设置 `c_int` 为 i32
pub fn sys_set_tid_address(tidptr: *const i32) -> KResult {
    let thread = local_hart().curr_thread();
    thread.lock_inner_with(|inner| inner.clear_child_tid = tidptr as usize);
    Ok(thread.tid() as isize)
}

/// 返回系统信息，返回值为 0
pub fn sys_uname(utsname: UserCheck<UtsName>) -> KResult {
    let utsname = unsafe { utsname.check_ptr_mut()? };
    utsname.write(UtsName::default());
    Ok(0)
}

/// 返回进程组号
///
/// TODO: 暂时未实现
pub fn sys_setpgid(_pid: usize, _pgid: usize) -> KResult {
    debug!("set pgid of {_pid} to {_pgid}");
    Ok(INITPROC.pid() as isize)
}

/// 返回进程组号
///
/// TODO: 暂时未实现，仅返回 INITPROC 的 pid
pub fn sys_getpgid(_pid: usize) -> KResult {
    debug!("get pgid of {_pid}");
    Ok(INITPROC.pid() as isize)
}
