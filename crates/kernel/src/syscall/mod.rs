mod flags;
mod fs;
mod process;
mod sync;
mod thread;

use defines::{error::errno, syscall::*};
use fs::*;
use process::*;
// use sync::*;
use thread::*;

use crate::{hart::curr_process, process::exit_process};

pub async fn syscall(id: usize, args: [usize; 6]) -> isize {
    let curr_pid = curr_process().pid();
    // 读入标准输入、写入标准输出、写入标准错误、INITPROC 和 shell 都不关心
    if (id == READ || id == READV) && args[0] == 0
        || (id == WRITE || id == WRITEV) && (args[0] == 1 || args[0] == 2)
        || id == PPOLL
    // || curr_pid == 1
    // || curr_pid == 2
    {
        trace!("args {args:x?}",);
    } else {
        info!("args {args:x?}",);
    }
    let ret = match id {
        // GETCWD => sys_getcwd(args[0] as _, args[1]),
        // DUP => sys_dup(args[0]),
        // DUP3 => sys_dup3(args[0], args[1]),
        // FCNTL64 => sys_fcntl64(args[0], args[1], args[2]),
        // IOCTL => sys_ioctl(args[0], args[1], args[2]),
        // MKDIRAT => sys_mkdirat(args[0], args[1] as _, args[2]),
        // UNLINKAT => sys_unlinkat(args[0], args[1] as _, args[2] as _),
        // LINKAT => sys_linkat(args[1] as _, args[3] as _),
        // UMOUNT => sys_umount(args[0] as _, args[1] as _),
        // MOUNT => sys_mount(
        //     args[0] as _,
        //     args[1] as _,
        //     args[2] as _,
        //     args[3],
        //     args[4] as _,
        // ),
        // CHDIR => sys_chdir(args[0] as _),
        // OPENAT => sys_openat(args[0], args[1] as _, args[2] as _, args[3] as _),
        // CLOSE => sys_close(args[0]),
        // PIPE2 => sys_pipe2(args[0] as _),
        // GETDENTS64 => sys_getdents64(args[0], args[1] as _, args[2]),
        READ => sys_read(args[0], args[1] as _, args[2]).await,
        WRITE => sys_write(args[0], args[1], args[2]).await,
        // READV => sys_readv(args[0], args[1] as _, args[2]),
        // WRITEV => sys_writev(args[0], args[1] as _, args[2]),
        // PPOLL => sys_ppoll(),
        // NEWFSTATAT => sys_fstatat(args[0], args[1] as _, args[2] as _, args[3]),
        // NEWFSTAT => sys_fstat(args[0], args[1] as _),
        EXIT => sys_exit(args[0] as _),
        EXIT_GROUP => sys_exit_group(args[0] as _),
        // SET_TID_ADDRESS => sys_set_tid_address(args[0] as _),
        // SLEEP => sys_sleep(args[0] as _),
        CLOCK_GETTIME => sys_clock_gettime(args[0] as _, args[1] as _),
        SCHED_YIELD => sys_sched_yield().await,
        // SIGACTION => sys_sigaction(args[0], args[1] as _, args[2] as _),
        // SIGPROCMASK => sys_sigprocmask(args[0], args[1] as _, args[2] as _, args[3]),
        SETPRIORITY => sys_setpriority(args[0] as _),
        TIMES => sys_times(args[0] as _),
        SETPGID => sys_setpgid(args[0], args[1]),
        GETPGID => sys_getpgid(args[0]),
        UNAME => sys_uname(args[0] as _),
        GETPID => sys_getpid(),
        GETPPID => sys_getppid(),
        GETUID | GETEUID | GETGID | GETEGID => Ok(0), // TODO: 目前不实现用户和用户组相关的部分
        GETTID => sys_gettid(),
        // BRK => sys_brk(args[0]),
        MUNMAP => sys_munmap(args[0], args[1]),
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        EXECVE => sys_execve(args[0] as _, args[1] as _, args[2] as _),
        WAIT4 => sys_wait4(args[0] as _, args[1] as _, args[2], args[3]).await,
        GET_TIME => sys_get_time_of_day(args[0] as _, args[1]),
        MMAP => sys_mmap(
            args[0],
            args[1],
            args[2] as _,
            args[3] as _,
            args[4] as _,
            args[5],
        ),
        _ => {
            error!("Unsupported syscall id: {id}");
            exit_process(curr_process(), -10);
            Ok(0)
        }
    };
    match ret {
        Ok(ret) => {
            // TODO: 由于目前 WAIT4 实现原因，忽略 INITPROC 的 SCHED_YIELD 和 WAIT4
            if !(id == SCHED_YIELD && curr_pid == 1) {
                // shell 的 stdin 和 stdout 以较低等级输出
                if (id == READ || id == WRITE) && curr_pid == 2 {
                    trace!("return {ret} = {ret:#x}");
                } else {
                    info!("return {ret} = {ret:#x}",);
                }
            }
            ret
        }
        Err(err) => {
            // 等待进程的 EAGAIN 可以忽视
            if !(id == WAIT4 && err == errno::EAGAIN) {
                info!("return {err:?}, {}", errno::error_info(err.as_isize()),);
            }
            err.as_isize()
        }
    }
}
