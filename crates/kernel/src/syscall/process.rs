//! Process management syscalls

use alloc::vec::Vec;
use compact_str::CompactString;
use defines::{
    constant::{MICRO_PER_SEC, NANO_PER_SEC},
    error::{errno, Result},
    structs::{TimeSpec, TimeVal, UtsName},
};
use memory::VirtAddr;
use user_check::UserCheck;

use crate::{
    hart::{curr_process, local_hart},
    process::exit_process,
    syscall::flags::{CloneFlags, MmapFlags, MmapProt, WaitFlags},
};

// TODO: 退出需要给其父进程发送 `SIGCHLD` 信号

/// 退出当前线程，结束用户线程循环。
pub fn sys_exit(exit_code: i32) -> Result {
    unsafe {
        (*local_hart())
            .curr_thread()
            .lock_inner(|inner| inner.exit_code = (exit_code & 0xff) as i8);
    };
    Err(errno::BREAK)
}

/// 退出进程，退出所有线程
pub fn sys_exit_group(exit_code: i32) -> Result {
    exit_process(
        unsafe { (*local_hart()).curr_process() },
        (exit_code & 0xff) as i8,
    );
    Err(errno::BREAK)
}

/// 挂起当前任务，让出 CPU，永不失败
pub async fn sys_sched_yield() -> Result {
    unsafe { (*local_hart()).curr_thread().yield_now().await }
    Ok(0)
}

/// 返回当前进程 id，永不失败
pub fn sys_getpid() -> Result {
    Ok(curr_process().pid() as isize)
}

/// 返回当前进程的父进程的 id，永不失败
pub fn sys_getppid() -> Result {
    Ok(curr_process().lock_inner_with(|inner| inner.parent.upgrade().unwrap().pid() as isize))
}

/// 创建子任务，通过 flags 进行精确控制。父进程返回子进程 pid，子进程返回 0。
///
/// TODO: 完善 `sys_clone()` 及文档
///
/// 参数：
/// - `flags` 指定 clone 的方式。具体参看 [`CloneFlags`]
pub fn sys_clone(
    flags: usize,
    user_stack: usize,
    _ptid: usize,
    _tls: usize,
    _ctid: usize,
) -> Result {
    if u32::try_from(flags).is_err() {
        error!("flags exceeds u32: {flags:#b}");
        return Err(errno::UNSUPPORTED);
    }
    // 参考 https://man7.org/linux/man-pages/man2/clone.2.html，低 8 位是 exit_signal，其余是 clone flags
    let Some(clone_flags) = CloneFlags::from_bits((flags as u32) & !0xff) else {
        error!("undefined CloneFlags: {:#b}", flags & !0xff);
        return Err(errno::UNSUPPORTED);
    };
    // TODO: 完成 exit_signal
    // let Ok(_exit_signal) = Signal::try_from(flags as u8) else {
    //     error!("未定义的信号：{:#b}", flags as u8);
    //     return Err(errno::UNSUPPORTED);
    // };
    if !clone_flags.is_empty() {
        error!("CloneFlags unsupported: {clone_flags:?}");
        return Err(errno::UNSUPPORTED);
    }
    let current_process = curr_process();
    let new_process = current_process.fork(user_stack);
    Ok(new_process.pid() as isize)
}

/// 将当前进程的地址空间清空并加载一个特定的可执行文件，返回用户态后开始它的执行。返回参数个数
///
/// 参数：
/// - `pathname` 给出了要加载的可执行文件的名字，必须以 `\0` 结尾
/// - `argv` 给出了参数列表。其最后一个元素必须是 0
/// - `envp` 给出环境变量列表，其最后一个元素必须是 0，目前未实现
pub fn sys_execve(pathname: *const u8, mut argv: *const usize, envp: *const usize) -> Result {
    assert!(envp.is_null(), "envp 暂时尚未支持");
    let pathname = UserCheck::new(pathname as _).check_cstr()?;
    trace!("pathname: {}", &*pathname);
    // 收集参数列表
    let mut arg_vec: Vec<CompactString> = Vec::new();
    unsafe {
        while *argv != 0 {
            let arg_str_ptr = UserCheck::new(argv as *mut usize).check_ptr()?;
            let arg_str = UserCheck::new(*arg_str_ptr as _).check_cstr()?;
            arg_vec.push(CompactString::from(&*arg_str));
            argv = argv.add(1);
        }
    }
    // 执行新进程
    let process = curr_process();

    let argc = arg_vec.len();
    process.exec(CompactString::from(&*pathname), arg_vec)?;
    Ok(argc as isize)
}

/// 挂起本线程，等待子进程改变状态（终止、或信号处理）。默认而言，会阻塞式等待子进程终止。
///
/// 若成功，返回子进程 pid，若 `options` 指定了 WNOHANG 且子线程存在但状态为改变，则返回 0
///
/// TODO: 信号处理的部分暂未实现
///
/// 参数：
/// - `pid` 要等待的 pid
///     - `pid` < -1，则等待一个 pgid 为 `pid` 绝对值的子进程，目前不支持
///     - `pid` == -1，则等待任意一个子进程
///     - `pid` == 0，则等待一个 pgid 与调用进程**调用时**的 pgid 相同的子进程，目前不支持
///     - `pid` > 0，则等待指定 `pid` 的子进程
/// - `wstatus: *mut i32` 指向一个 int，若非空则用于表示某些状态，目前而言似乎仅需往里写入子进程的 exit code
/// - `options` 控制等待方式，详细查看 [`WaitFlags`]，目前只支持 `WNOHANG`
/// - `rusgae` 用于统计子进程资源使用情况，目前不支持
pub async fn sys_wait4(pid: isize, wstatus: usize, options: usize, rusage: usize) -> Result {
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

    // 尝试寻找符合条件的子进程
    loop {
        // 尝试找到一个符合条件，且已经是僵尸的子进程
        let listener = {
            // 用块是因为 rust 目前不够聪明。
            // inner 是个 Guard，不 Send，因此不能包含在 future 中
            // 但在同一块作用域中，即使 drop inner，也依然会导致 future 不 send，非常麻烦
            // 这也导致 listener 不得不堆分配，而暂时无法用栈上的 listener
            let process = curr_process();
            let mut inner = process.lock_inner();
            let mut has_proper_child = false;
            let mut child_index = None;
            for (index, child) in inner.children.iter().enumerate() {
                if pid == -1 || child.pid() == pid as usize {
                    has_proper_child = true;
                    if child.lock_inner_with(|inner| inner.threads.is_empty()) {
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
                let exit_code = child.lock_inner_with(|inner| inner.exit_code.unwrap());
                let wstatus = wstatus as *mut i32;
                if !wstatus.is_null() {
                    let mut wstatus = UserCheck::new(wstatus).check_ptr_mut()?;
                    // *wstatus 的构成，可能要参考 WEXITSTATUS 那几个宏
                    *wstatus = (exit_code as i32) << 8;
                }
                return Ok(found_pid as isize);
            }

            // 否则视 `options` 而定
            if options.contains(WaitFlags::WNOHANG) {
                return Ok(0);
            }
            process.wait4_event.listen()
        };

        trace!("no proper child exited");
        listener.await;
    }
}

/// 获取自 Epoch 以来所过的时间（不过目前实现中似乎是自开机或复位以来时间）
///
/// 参数：
/// - `ts` 要设置的时间值
/// - `tz` 时区结构，但目前已经过时，不考虑
pub fn sys_get_time_of_day(tv: *mut TimeVal, _tz: usize) -> Result {
    // 根据 man 所言，时区参数 tz 已经过时了，通常应当是 NULL。
    assert_eq!(_tz, 0);
    let mut tv = UserCheck::new(tv).check_ptr_mut()?;
    let us = riscv_time::get_time_us();
    tv.sec = us / MICRO_PER_SEC;
    tv.usec = us % MICRO_PER_SEC;
    Ok(0)
}

/// 全局时钟，或者说挂钟
const CLOCK_REALTIME: usize = 0;

/// 同样是获取时间，不过 `TimeSpec` 精度为 ns。
///
/// 可以有不同的时钟，但目前只支持挂钟 (`CLOCK_REALTIME`)。
///
/// 参数：
/// - `clock_id` 时钟 id，目前仅为 `CLOCK_READTIME`
/// - `tp` 指向要设置的用户指针
pub fn sys_clock_gettime(_clock_id: usize, ts: *mut TimeSpec) -> Result {
    // TODO: 目前只考虑挂钟时间
    assert_eq!(_clock_id, CLOCK_REALTIME);
    let mut ts = UserCheck::new(ts).check_ptr_mut()?;
    let us = riscv_time::get_time_ns();
    ts.sec = us / NANO_PER_SEC;
    ts.nsec = us % NANO_PER_SEC;
    Ok(0)
}

pub fn sys_setpriority(_prio: isize) -> Result {
    todo!("[low] sys_setpriority")
}

#[repr(C)]
pub struct Tms {
    /// 当前进程的用户态时间
    tms_utime: usize,
    /// 当前进程的内核态时间
    tms_stime: usize,
    /// 已被 wait 的子进程的用户态时间
    tms_cutime: usize,
    /// 已被 wait 的子进程的内核态时间
    tms_cstime: usize,
}

// FIXME: `sys_times` 暂时是非正确的实现
/// 获取进程和子进程运行时间，单位是**时钟 tick 数**
///
/// 参数：
/// - `tms` 是一个用户指针，结果被写入其中。
///
/// 错误：
/// - `EFAULT` `tms` 指向非法地址
pub fn sys_times(tms: *mut Tms) -> Result {
    let ticks = riscv::register::time::read();
    let mut tms = UserCheck::new(tms).check_ptr_mut()?;
    tms.tms_utime = ticks / 4;
    tms.tms_stime = ticks / 4;
    tms.tms_cutime = ticks / 4;
    tms.tms_cstime = ticks / 4;
    Ok(0)
}

/// 映射虚拟内存。返回实际映射的地址。
///
/// `addr` 若是 NULL，那么内核会自动选择一个按页对齐的地址进行映射，这也是比较可移植的方式。
///
/// `addr` 若有指定地址，那么内核会尝试在最近的页边界上映射，但如果已经被映射过了，
/// 就挑选一个新的地址。该新地址可能参考也可能不参考 `addr`。
///
/// 如果映射文件，那么会以该文件 (`fd`) `offset` 开始处的 `len` 个字节初始化映射内容。
///
/// `mmap()` 返回之后，就算 `fd` 指向的文件被立刻关闭，也不会影响映射的结果
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
pub fn sys_mmap(addr: usize, len: usize, prot: u32, flags: u32, fd: i32, offset: usize) -> Result {
    info!("addr: {addr}");
    info!("len: {len}");

    if VirtAddr(addr).page_offset() != 0 || len == 0 {
        return Err(errno::EINVAL);
    }
    let Some(prot) = MmapProt::from_bits(prot) else {
        // prot 出现了意料之外的标志位
        error!("prot: {prot:#b}");
        return Err(errno::UNSUPPORTED);
    };
    let Some(flags) = MmapFlags::from_bits(flags) else {
        // flags 出现了意料之外的标志位
        error!("flags: {flags:#b}");
        return Err(errno::UNSUPPORTED);
    };
    info!("prot: {prot:?}");
    info!("flags: {flags:?}");
    info!("fd: {fd}");
    info!("offset: {offset}");
    if flags.contains(MmapFlags::MAP_ANONYMOUS | MmapFlags::MAP_SHARED) {
        error!("anonymous shared mapping is not supported!");
        return Err(errno::EPERM);
    }
    if flags.contains(MmapFlags::MAP_ANONYMOUS) {
        if fd != -1 || offset != 0 {
            error!("fd must be -1 and offset must be 0 for anonyous mapping");
            return Err(errno::EPERM);
        }
        let process = curr_process();
        info!("pid: {}", process.pid());
        // TODO: [blocked] 还没有处理 MmapFlags::MAP_FIXED 的情况？
        return process.lock_inner_with(|inner| {
            inner.memory_set.try_map(
                VirtAddr(addr).vpn()..VirtAddr(addr + len).vpn(),
                prot.into(),
                false,
            )
        });
    }

    // TODO: [blocked] 其他映射尚未实现
    Err(errno::UNSUPPORTED)
}

pub fn sys_munmap(_addr: usize, _len: usize) -> Result {
    // Err(errno::UNSUPPORTED)
    todo!("[blocked] sys_munmap")
}

// /// 设置线程控制块中 `clear_child_tid` 的值为 `tidptr`。总是返回调用者线程的 tid。
// ///
// /// 参数：
// /// - `tidptr`
// pub fn sys_set_tid_address(tidptr: *const i32) -> Result {
//     // NOTE: 在 linux 手册中，`tidptr` 的类型是 int*。这里设置为 i32，是参考 libc crate 设置 c_int=i32
//     let thread = curr_task().unwrap();
//     let mut inner = thread.inner();
//     inner.clear_child_tid = tidptr as usize;
//     Ok(inner.res.as_ref().unwrap().tid as isize)
//     todo!("[mid] sys_set_tid_address")
// }

// /// 将 program break 设置为 `brk`。高于当前堆顶会分配空间，低于则会释放空间。
// ///
// /// `brk` 为 0 时返回当前堆顶地址。设置成功时返回新的 brk，设置失败返回原来的 brk
// ///
// /// 参数：
// /// - `brk` 希望设置的 program break 值
// pub fn sys_brk(brk: usize) -> Result {
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // // 不大于最初的堆地址则失败。其中也包括了 brk 为 0  的情况
//     // Ok(inner.set_user_brk(brk) as isize)
//     todo!("[mid] sys_brk")
// }

// /// 为当前进程设置信号动作，返回 0
// ///
// /// 参数：
// /// - `signum` 指示信号编号，但不可以是 `SIGKILL` 或 `SIGSTOP`
// /// - `act` 如果非空，则将信号 `signal` 的动作设置为它
// /// - `old_act` 如果非空，则将信号 `signal` 原来的动作备份在其中
// pub fn sys_sigaction(
//     signum: usize,
//     act: *const SignalAction,
//     old_act: *mut SignalAction,
// ) -> Result {
//     let signal = Signal::try_from_primitive(signum as u8).or(Err(errno::EINVAL))?;
//     // `SIGKILL` 和 `SIGSTOP` 的行为不可修改
//     if matches!(signal, Signal::SIGKILL | Signal::SIGSTOP) {
//         return Err(errno::EINVAL);
//     }
//     let process = curr_process();
//     let mut inner = process.inner();

//     if !old_act.is_null() {
//         let old_act = unsafe { check_ptr_mut(old_act)? };
//         *old_act = inner.sig_handlers.action(signal);
//     }

//     if !act.is_null() {
//         let act = unsafe { check_ptr(act)? };
//         inner.sig_handlers.set_action(signal, *act);
//     }

//     Ok(0)
// }

// /// 修改当前线程的信号掩码，返回 0
// ///
// /// 参数：
// /// - `how` 只应取 0(`SIG_BLOCK`)、1(`SIG_UNBLOCK`)、2(`SIG_SETMASK`)，表示函数的处理方式。
// ///     - `SIG_BLOCK` 向掩码 bitset 中添入新掩码
// ///     - `SIG_UNBLOCK` 从掩码 bitset 中取消掩码
// ///     - `SIG_SETMASK` 直接设置掩码 bitset
// /// - `set` 为空时，信号掩码不会被修改（无论 `how` 取何值）。其余时候则是新掩码参数，根据 `how` 进行设置
// /// - `old_set` 非空时，将旧掩码的值放入其中
// pub fn sys_sigprocmask(
//     how: usize,
//     set: *const SignalSet,
//     old_set: *mut SignalSet,
//     sigsetsize: usize,
// ) -> Result {
//     // NOTE: 这里 `set` == `old_set` 的情况是否需要考虑一下
//     if sigsetsize != SIGSET_SIZE_BYTES {
//         return Err(errno::EINVAL);
//     }
//     let thread = curr_task().unwrap();
//     let mut inner = thread.inner();

//     let sig_set = &mut inner.sig_receiver.mask;
//     if !old_set.is_null() {
//         let old_set = unsafe { check_ptr_mut(old_set)? };
//         *old_set = *sig_set;
//     }
//     if set.is_null() {
//         return Ok(0);
//     }
//     const SIG_BLOCK: usize = 0;
//     const SIG_UNBLOCK: usize = 1;
//     const SIG_SETMASK: usize = 2;

//     let set = unsafe { check_ptr(set)? };
//     match how {
//         SIG_BLOCK => {
//             sig_set.insert(*set);
//         }
//         SIG_UNBLOCK => {
//             sig_set.remove(*set);
//         }
//         SIG_SETMASK => {
//             *sig_set = *set;
//         }
//         _ => return Err(errno::EINVAL),
//     }

//     Ok(0)
// }

/// 返回系统信息，返回值为 0
pub fn sys_uname(utsname: *mut UtsName) -> Result {
    let mut utsname = UserCheck::new(utsname).check_ptr_mut()?;
    *utsname = UtsName::default();
    Ok(0)
}

/// 设置进程组号
///
/// TODO: 暂时未实现，仅返回 0
pub fn sys_setpgid(_pid: usize, _pgid: usize) -> Result {
    Ok(0)
}

/// 返回进程组号
///
/// TODO: 暂时未实现，仅返回 0
pub fn sys_getpgid(_pid: usize) -> Result {
    Ok(0)
}
