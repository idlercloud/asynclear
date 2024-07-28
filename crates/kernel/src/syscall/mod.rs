mod fs;
mod memory;
mod misc;
mod process;
mod signal;
mod thread;
mod time;

use defines::{
    error::{errno, KResult},
    syscall::*,
};
use fs::*;
use memory::*;
use misc::*;
use process::*;
use signal::*;
use thread::*;
use time::*;

use crate::{hart::local_hart, memory::UserCheck, process::exit_process};

pub async fn syscall(id: usize, args: [usize; 6]) -> isize {
    // 读入标准输入、写入标准输出、写入标准错误都不关心
    // busybox sh 会频繁调用 PPOLL
    // 一些特别简单基本不太会维护的也可以不关心
    let is_trace = (id == READ || id == READV) && args[0] == 0
        || (id == WRITE || id == WRITEV) && (args[0] == 1 || args[0] == 2)
        || (id == PPOLL && args[1] == 1)
        || [
            GETPGID, GETPID, GETPPID, GETUID, GETUID, SETPGID, GETEUID, UNAME, EXIT, EXIT_GROUP,
            GETTID,
        ]
        .contains(&id);
    // 一些比较成熟的 syscall 也可以适当降低日志等级
    let is_debug = [
        NEWFSTATAT,
        IOCTL,
        WAIT4,
        BRK,
        GETCWD,
        RT_SIGACTION,
        RT_SIGPROCMASK,
        RT_SIGRETURN,
        FCNTL64,
        OPENAT,
        CLOSE,
    ]
    .contains(&id);
    let ret = syscall_impl(id, args).await;
    match ret {
        Ok(ret) => {
            if is_trace {
                trace!("args {args:x?}. return {ret} = {ret:#x}");
            } else if is_debug {
                debug!("args {args:x?}. return {ret} = {ret:#x}");
            } else {
                info!("args {args:x?}. return {ret} = {ret:#x}");
            }
            ret as isize
        }
        Err(err) => {
            // 等待进程的 EAGAIN 和 ECHILD 可以忽视
            if !((id == WAIT4 && (err == errno::EAGAIN || err == errno::ECHILD))
                || err == errno::BREAK)
            {
                warn!(
                    "args {args:x?}. return {err:?}, {}",
                    errno::error_info(err.as_isize()),
                );
            }
            err.as_isize()
        }
    }
}

async fn syscall_impl(id: usize, args: [usize; 6]) -> KResult {
    match id {
        GETCWD => sys_getcwd(UserCheck::new_slice(args[0] as _, args[1]).ok_or(errno::EINVAL)?),
        DUP => sys_dup(args[0]),
        DUP3 => sys_dup3(args[0], args[1], args[2] as _),
        FCNTL64 => sys_fcntl64(args[0], args[1], args[2]),
        IOCTL => sys_ioctl(args[0], args[1], args[2]),
        MKDIRAT => sys_mkdirat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            args[2],
        ),
        UNLINKAT => sys_unlinkat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            args[2] as _,
        ),
        // LINKAT => sys_linkat(args[1] as _, args[3] as _),
        UMOUNT => sys_umount(
            UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?,
            args[1] as _,
        ),
        MOUNT => sys_mount(
            UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?,
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            UserCheck::new(args[2] as _).ok_or(errno::EINVAL)?,
            args[3] as _,
            UserCheck::new(args[4] as _),
        ),
        STATFS64 => sys_statfs64(
            UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?,
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
        ),
        FACCESSAT => sys_faccessat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            args[2] as _,
        ),
        CHDIR => sys_chdir(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?),
        OPENAT => sys_openat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            args[2] as _,
            args[3] as _,
        ),
        CLOSE => sys_close(args[0]),
        PIPE2 => sys_pipe2(
            UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?,
            args[1] as _,
        ),
        GETDENTS64 => sys_getdents64(
            args[0],
            UserCheck::new_slice(args[1] as _, args[2]).ok_or(errno::EINVAL)?,
        ),
        LSEEK => sys_lseek(args[0], args[1] as _, args[2]).await,
        READ => {
            sys_read(
                args[0],
                UserCheck::new_slice(args[1] as _, args[2]).ok_or(errno::EINVAL)?,
            )
            .await
        }
        WRITE => {
            sys_write(
                args[0],
                UserCheck::new_slice(args[1] as _, args[2]).ok_or(errno::EINVAL)?,
            )
            .await
        }
        READV => {
            sys_readv(
                args[0],
                UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
                args[2],
            )
            .await
        }
        WRITEV => {
            sys_writev(
                args[0],
                UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
                args[2],
            )
            .await
        }
        SENDFILE64 => sys_sendfile64(args[0], args[1], UserCheck::new(args[2] as _), args[3]).await,
        PPOLL => sys_ppoll(
            UserCheck::new_slice(args[0] as _, args[1]).ok_or(errno::EINVAL)?,
            UserCheck::new(args[2] as _),
            UserCheck::new(args[3] as _),
            args[4],
        ),
        NEWFSTATAT => sys_newfstatat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            UserCheck::new(args[2] as _).ok_or(errno::EINVAL)?,
            args[3],
        ),
        NEWFSTAT => sys_newfstat(args[0], UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?),
        UTIMENSAT => sys_utimensat(
            args[0],
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
            UserCheck::new(args[2] as _),
            args[3],
        ),
        EXIT => sys_exit(args[0] as _),
        EXIT_GROUP => sys_exit_group(args[0] as _),
        SET_TID_ADDRESS => sys_set_tid_address(args[0] as _),
        NANOSLEEP => sys_nanosleep(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?).await,
        CLOCK_GETTIME => sys_clock_gettime(
            args[0] as _,
            UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
        ),
        SYSLOG => sys_syslog(
            args[0] as _,
            UserCheck::new_slice(args[1] as _, args[2]).ok_or(errno::EINVAL)?,
        ),
        SCHED_YIELD => sys_sched_yield().await,
        KILL => sys_kill(args[0] as _, args[1]),
        RT_SIGACTION => sys_rt_sigaction(
            args[0],
            UserCheck::new(args[1] as _),
            UserCheck::new(args[2] as _),
        ),
        RT_SIGPROCMASK => sys_rt_sigprocmask(
            args[0],
            UserCheck::new(args[1] as _),
            UserCheck::new(args[2] as _),
            args[3],
        ),
        RT_SIGRETURN => sys_rt_sigreturn(),
        SETPRIORITY => sys_setpriority(args[0] as _),
        TIMES => sys_times(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?),
        SETPGID => sys_setpgid(args[0], args[1]),
        GETPGID => sys_getpgid(args[0]),
        UNAME => sys_uname(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?),
        GET_TIME_OF_DAY => {
            sys_get_time_of_day(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?, args[1])
        }
        GETPID => sys_getpid(),
        GETPPID => sys_getppid(),
        GETUID | GETEUID | GETGID | GETEGID => Ok(0), // TODO: 目前不实现用户和用户组相关的部分
        GETTID => sys_gettid(),
        SYSINFO => sys_sysinfo(UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?),
        BRK => sys_brk(args[0]),
        MUNMAP => sys_munmap(args[0], args[1]),
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        EXECVE => {
            sys_execve(
                UserCheck::new(args[0] as _).ok_or(errno::EINVAL)?,
                UserCheck::new(args[1] as _).ok_or(errno::EINVAL)?,
                UserCheck::new(args[2] as _),
            )
            .await
        }
        MMAP => sys_mmap(
            args[0],
            args[1],
            args[2] as _,
            args[3] as _,
            args[4] as _,
            args[5],
        ),
        WAIT4 => sys_wait4(args[0] as _, UserCheck::new(args[1] as _), args[2], args[3]).await,
        _ => {
            error!("Unsupported syscall id: {id}");
            exit_process(&local_hart().curr_process(), -10);
            Ok(0)
        }
    }
}
