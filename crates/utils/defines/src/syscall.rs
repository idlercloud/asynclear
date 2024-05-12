macro_rules! declare_syscall_id {
    ($($name:tt, $id:literal,)*) => {
        $(pub const $name: usize = $id;)*
        pub fn name(id: usize) -> &'static str {
            match id {
                $($id => stringify!($name),)*
                _ => unreachable!("{}", id),
            }
        }
    };
}

#[rustfmt::skip]
declare_syscall_id!(
    GETCWD,             17,
    // DUP,                23,
    DUP3,               24,
    FCNTL64,            25,
    IOCTL,              29,
    MKDIRAT,            34,
    // UNLINKAT,           35,
    // LINKAT,             37,
    // UMOUNT,             39,
    // MOUNT,              40,
    CHDIR,              49,
    OPENAT,             56,
    CLOSE,              57,
    // PIPE2,              59,
    GETDENTS64,         61,
    READ,               63,
    WRITE,              64,
    READV,              65,
    WRITEV,             66,
    PPOLL,              73,
    NEWFSTATAT,         79,
    // NEWFSTAT,           80,
    EXIT,               93,
    EXIT_GROUP,         94,
    SET_TID_ADDRESS,    96,
    // SLEEP,              101,
    CLOCK_GETTIME,      113,
    SCHED_YIELD,        124,
    // KILL,               129,
    RT_SIGACTION,       134,
    RT_SIGPROCMASK,     135,
    RT_SIGRETURN,       139,
    SETPRIORITY,        140,
    TIMES,              153,
    SETPGID,            154,
    GETPGID,            155,
    UNAME,              160,
    GET_TIME,           169,
    GETPID,             172,
    GETPPID,            173,
    GETUID,             174,
    GETEUID,            175,
    GETGID,             176,
    GETEGID,            177,
    GETTID,             178,
    BRK,                214,
    MUNMAP,             215,
    CLONE,              220,
    EXECVE,             221,
    MMAP,               222,
    WAIT4,              260,
);
