use core::ffi::CStr;

use defines::{
    fs::Stat,
    misc::{MmapFlags, MmapProt, TimeSpec, UtsName},
    signal::KSignalAction,
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

pub fn sys_openat(dirfd: usize, path: &CStr, flags: u32, mode: u32) -> isize {
    syscall6(
        OPENAT,
        [
            dirfd,
            path.as_ptr() as usize,
            flags as usize,
            mode as usize,
            0,
            0,
        ],
    )
}

pub fn sys_close(fd: i32) -> isize {
    syscall3(CLOSE, [fd as usize, 0, 0])
}

pub fn sys_lseek(fd: i32, offset: i64, whence: usize) -> isize {
    syscall3(LSEEK, [fd as usize, offset as usize, whence])
}

pub fn sys_read(fd: i32, buffer: &mut [u8]) -> isize {
    syscall3(
        READ,
        [fd as usize, buffer.as_mut_ptr() as usize, buffer.len()],
    )
}

pub fn sys_write(fd: i32, buffer: *const u8, len: usize) -> isize {
    syscall3(WRITE, [fd as usize, buffer as usize, len])
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

pub fn sys_newfstat(fd: usize, st: &mut Stat) -> isize {
    syscall3(NEWFSTAT, [fd, st as *mut _ as usize, 0])
}

pub fn sys_exit(exit_code: i32) -> ! {
    syscall3(EXIT, [exit_code as usize, 0, 0]);
    unreachable!("sys_exit never returns!");
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

pub fn sys_clone(flags: usize) -> isize {
    syscall3(CLONE, [flags, 0, 0])
}

/// # Safety
///
/// 需保证 alias 及类型安全
pub unsafe fn sys_execve(path: *const u8, args: *const *const u8) -> isize {
    syscall3(EXECVE, [path as usize, args as usize, 0])
}

pub fn sys_waitpid(pid: isize, xstatus: *mut i32) -> isize {
    syscall6(WAIT4, [pid as usize, xstatus as usize, 0, 0, 0, 0])
}

pub fn sys_set_priority(prio: isize) -> isize {
    syscall3(SETPRIORITY, [prio as usize, 0, 0])
}

pub fn sys_mmap(
    start: usize,
    len: usize,
    prot: MmapProt,
    flags: MmapFlags,
    fd: usize,
    offset: usize,
) -> isize {
    syscall6(
        MMAP,
        [
            start,
            len,
            prot.bits() as usize,
            flags.bits() as usize,
            fd,
            offset,
        ],
    )
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    syscall3(MUNMAP, [start, len, 0])
}

pub fn sys_chdir(path: &CStr) -> isize {
    syscall3(CHDIR, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_getcwd(buf: &mut [u8]) -> isize {
    syscall3(GETCWD, [buf.as_ptr() as usize, buf.len(), 0])
}

pub fn sys_getdents64(fd: usize, buf: &mut [u8]) -> isize {
    syscall3(GETDENTS64, [fd, buf.as_ptr() as usize, buf.len()])
}

pub fn sys_dup(fd: usize) -> isize {
    syscall3(DUP, [fd, 0, 0])
}

pub fn sys_pipe(pipe_fd: &mut [i32; 2], flags: u32) -> isize {
    syscall3(PIPE2, [pipe_fd.as_mut_ptr() as usize, flags as usize, 0])
}

pub fn sys_gettid() -> isize {
    syscall3(GETTID, [0; 3])
}

/// 返回系统信息，返回值为 0
///
/// # Safety
///
/// 需保证 alias 及类型安全
pub unsafe fn sys_uname(utsname: *mut UtsName) -> isize {
    syscall3(UNAME, [utsname as _, 0, 0])
}

pub fn sys_rt_sigaction(
    signum: usize,
    act: *const KSignalAction,
    old_act: *mut KSignalAction,
) -> isize {
    syscall3(RT_SIGACTION, [signum, act as _, old_act as _])
}

pub fn sys_brk(brk: usize) -> isize {
    syscall3(BRK, [brk, 0, 0])
}

pub fn sys_clock_gettime(clock_id: usize, ts: *mut TimeSpec) -> isize {
    syscall3(CLOCK_GETTIME, [clock_id, ts as usize, 0])
}

pub fn sys_iotcl(fd: usize, request: usize, argp: usize) -> isize {
    syscall3(IOCTL, [fd, request, argp])
}
