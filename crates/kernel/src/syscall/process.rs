//! Process management syscalls

use alloc::vec::Vec;
use core::num::NonZeroUsize;

use atomic::Ordering;
use common::config::PAGE_OFFSET_MASK;
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::{CloneFlags, MmapFlags, MmapProt, UtsName, WaitFlags},
    signal::Signal,
};
use event_listener::listener;
use triomphe::Arc;

use crate::{
    executor,
    hart::local_hart,
    memory::{MapPermission, UserCheck, VirtAddr},
    process::{exit_process, INITPROC},
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
        if Signal::try_from((flags as u8).wrapping_sub(1)).is_ok() {
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
            let Ok(signal) = Signal::try_from((flags as u8).wrapping_sub(1)) else {
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

/// 映射虚拟内存。返回实际映射的地址。
///
/// `addr` 若是 NULL，那么内核会自动选择一个按页对齐的地址进行映射，这也是比较可移植的方式。
///
/// `addr` 若有指定地址，那么内核会尝试在最近的页边界上映射，但如果已经被映射过了，就挑选一个新的地址。
/// 该新地址可能参考也可能不参考 `addr`。
///
/// 如果映射文件，那么会以该文件 (`fd`) `offset` 开始处的 `len` 个字节初始化映射内容。
///
/// `mmap()` 返回之后，就算 `fd` 指向的文件被立刻关闭，也不会影响映射的结果。
///
/// `prot` 要么是 `PROT_NONE`，要么是多个标志位的或。
///
/// `flags` 决定该映射是否对其他映射到同一区域的进程可见，以及更新是否会同步到底层文件上。
///
/// 参数：
/// - `addr` 映射的目标地址。
/// - `len` 映射的目标长度
/// - `prot` 描述该映射的内存保护方式，不能与文件打开模式冲突
/// - `flags` 描述映射的特征，详细参考 [`MmapFlags`]
/// - `fd` 被映射的文件描述符
/// - `offset` 映射的起始偏移，必须是 `PAGE_SIZE` 的整数倍
pub fn sys_mmap(addr: usize, len: usize, prot: u32, flags: u32, fd: i32, offset: usize) -> KResult {
    let prot = MmapProt::from_bits(prot).ok_or(errno::EINVAL)?;
    let Some(flags) = MmapFlags::from_bits(flags) else {
        // flags 出现了意料之外的标志位
        error!("unsupported flags: {flags:#b}");
        return Err(errno::UNSUPPORTED);
    };
    debug!("prot: {prot:?}, flags: {flags:?}");
    if flags.contains(MmapFlags::MAP_SHARED) {
        // 共享映射
        todo!("[mid] impl shared mapping");
    } else {
        // 私有映射
        // `MAP_SHARED`、`MAP_PRIVATE` 至少有其一
        if !flags.contains(MmapFlags::MAP_PRIVATE) {
            return Err(errno::EINVAL);
        }

        if flags.contains(MmapFlags::MAP_ANONYMOUS) {
            // 私有匿名映射
            if fd != -1 || offset != 0 {
                warn!("fd must be -1 and offset must be 0 for anonyous mapping");
                return Err(errno::EINVAL);
            }
            private_anonymous_map(prot, flags, addr, len)
        } else {
            todo!("[mid] impl private file mapping");
        }
    }
}

/// 私有匿名映射，没有底层文件。内容全部初始化为 0
///
/// 如果 addr 没有对齐到页边界或者 len 为 0
fn private_anonymous_map(prot: MmapProt, flags: MmapFlags, addr: usize, len: usize) -> KResult {
    debug!("private anonymous map, addr: {addr:#}, len: {len}");
    if addr & PAGE_OFFSET_MASK != 0 || len == 0 {
        return Err(errno::EINVAL);
    }
    let process = local_hart().curr_process();
    let va_start = VirtAddr(addr);
    process.lock_inner_with(|inner| {
        inner
            .memory_space
            .try_map(va_start..va_start + len, MapPermission::from(prot), flags)
    })
}

/// 将一块区域取消映射。
///
/// （未实现）有可能产生多个新的区域，比如 unmap 一个大区域的中间，左右两遍会变成两个单独的小区域
///
/// 在目前的实现中应该只会在参数不正确（`addr` 未对齐、`len` 为 0）时返回 `EINVAL` 一种错误
pub fn sys_munmap(addr: usize, len: usize) -> KResult {
    debug!("unmap {addr}..{}", addr + len);
    if addr & PAGE_OFFSET_MASK != 0 || len == 0 {
        return Err(errno::EINVAL);
    }
    let va_start = VirtAddr(addr);
    local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.memory_space.unmap(va_start..va_start + len));
    Ok(0)
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

/// 将 program break 设置为 `brk`。高于当前堆顶会分配空间，低于则会释放空间。
///
/// `brk` 为 0 时返回当前堆顶地址。设置成功时返回新的 brk，设置失败返回原来的 brk
///
/// 参数：
/// - `brk` 希望设置的 program break 值
pub fn sys_brk(brk: usize) -> KResult {
    let process = local_hart().curr_process();
    let mut inner = process.lock_inner();
    Ok(inner.set_user_brk(VirtAddr(brk)).0 as isize)
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
