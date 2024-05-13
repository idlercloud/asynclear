use defines::{
    misc::UtsName,
    signal::{KSignalAction, KSignalSet, SIGSET_SIZE_BYTES},
    syscall::*,
};

#[inline(always)]
pub fn syscall3(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}

#[inline(always)]
pub fn syscall4(id: usize, args: [usize; 4]) -> isize {
    let mut ret: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x13") args[3],
            in("x17") id
        );
    }
    ret
}

#[inline(always)]
pub fn syscall6(id: usize, args: [usize; 6]) -> isize {
    let mut ret: isize;
    unsafe {
        core::arch::asm!("ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x13") args[3],
            in("x14") args[4],
            in("x15") args[5],
            in("x17") id
        );
    }
    ret
}

// pub fn sys_openat(dirfd: usize, path: &str, flags: u32, mode: u32) -> isize {
//     syscall6(
//         OPENAT,
//         [
//             dirfd,
//             path.as_ptr() as usize,
//             flags as usize,
//             mode as usize,
//             0,
//             0,
//         ],
//     )
// }

// pub fn sys_close(fd: usize) -> isize {
//     syscall3(CLOSE, [fd, 0, 0])
// }

pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall3(READ, [fd, buffer.as_mut_ptr() as usize, buffer.len()])
}

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall3(WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

// pub fn sys_linkat(
//     old_dirfd: usize,
//     old_path: &str,
//     new_dirfd: usize,
//     new_path: &str,
//     flags: usize,
// ) -> isize {
//     syscall6(
//         LINKAT,
//         [
//             old_dirfd,
//             old_path.as_ptr() as usize,
//             new_dirfd,
//             new_path.as_ptr() as usize,
//             flags,
//             0,
//         ],
//     )
// }

// pub fn sys_unlinkat(dirfd: usize, path: &str, flags: usize) -> isize {
//     syscall3(UNLINKAT, [dirfd, path.as_ptr() as usize, flags])
// }

// pub fn sys_fstat(fd: usize, st: &Stat) -> isize {
//     syscall3(FSTAT, [fd, st as *const _ as usize, 0])
// }

pub fn sys_exit(exit_code: i32) -> ! {
    syscall3(EXIT, [exit_code as usize, 0, 0]);
    panic!("sys_exit never returns!");
}

pub fn sys_yield() -> isize {
    syscall3(SCHED_YIELD, [0, 0, 0])
}

pub fn sys_getpid() -> isize {
    syscall3(GETPID, [0, 0, 0])
}

pub fn sys_getppid() -> isize {
    syscall3(GETPPID, [0, 0, 0])
}

pub fn sys_clone4(flags: usize) -> isize {
    syscall3(CLONE, [flags, 0, 0])
}

pub fn sys_execve(path: &str, args: &[*const u8]) -> isize {
    syscall3(EXECVE, [path.as_ptr() as usize, args.as_ptr() as usize, 0])
}

pub fn sys_waitpid(pid: isize, xstatus: *mut i32) -> isize {
    syscall6(WAIT4, [pid as usize, xstatus as usize, 0, 0, 0, 0])
}

pub fn sys_set_priority(prio: isize) -> isize {
    syscall3(SETPRIORITY, [prio as usize, 0, 0])
}

pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    syscall3(MMAP, [start, len, prot])
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    syscall3(MUNMAP, [start, len, 0])
}

pub fn sys_chdir(path: &str) -> isize {
    syscall3(CHDIR, [path.as_ptr() as usize, 0, 0])
}

// pub fn sys_dup(fd: usize) -> isize {
//     syscall3(SYSCALL_DUP, [fd, 0, 0])
// }

// pub fn sys_pipe(pipe: &mut [usize]) -> isize {
//     syscall3(SYSCALL_PIPE, [pipe.as_mut_ptr() as usize, 0, 0])
// }

pub fn sys_gettid() -> isize {
    syscall3(GETTID, [0; 3])
}

/// 返回系统信息，返回值为 0
pub fn sys_uname(utsname: *mut UtsName) -> isize {
    syscall3(UNAME, [utsname as _, 0, 0])
}

pub fn sys_rt_sigaction(
    signum: usize,
    act: *const KSignalAction,
    old_act: *mut KSignalAction,
) -> isize {
    syscall3(RT_SIGACTION, [signum, act as _, old_act as _])
}

pub fn sys_rt_sigprocmask(how: usize, set: *const KSignalSet, old_set: *mut KSignalSet) -> isize {
    syscall4(
        RT_SIGPROCMASK,
        [how, set as _, old_set as _, SIGSET_SIZE_BYTES],
    )
}

pub fn sys_brk(brk: usize) -> isize {
    syscall3(BRK, [brk, 0, 0])
}
