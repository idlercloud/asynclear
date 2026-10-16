//! `sys_wait4` 写回的 `wstatus` 的解码函数，对应 Linux `sys/wait.h` 里的 `W*` 宏

// TODO: 目前内核尚未完整实现 wstatus 的编码

/// 子进程是正常退出的（`exit`/`_exit`/`main` 返回），即状态字低 7 位为 0
pub const fn wifexited(status: i32) -> bool {
    wtermsig(status) == 0
}

/// 子进程传给 `exit` 的退出码（最低 8 位，无符号）。只在 [`wifexited`] 为真时有意义
pub const fn wexitstatus(status: i32) -> u8 {
    ((status & 0xff00) >> 8) as u8
}

/// 子进程是被信号杀死的
pub const fn wifsignaled(status: i32) -> bool {
    let sig = wtermsig(status);
    sig != 0 && sig != 0x7f
}

/// 杀死子进程的信号编号。只在 [`wifsignaled`] 为真时有意义
pub const fn wtermsig(status: i32) -> u8 {
    (status & 0x7f) as u8
}

/// 子进程是因为信号而 core dump 的（只对 [`wifsignaled`] 为真的状态有意义）
pub const fn wcoredump(status: i32) -> bool {
    status & 0x80 != 0
}

/// 子进程被信号停止了（需要 `WUNTRACED` 或 ptrace 才可能观察到）
pub const fn wifstopped(status: i32) -> bool {
    (status & 0xff) == 0x7f
}

/// 子进程被 `SIGCONT` 继续了
pub const fn wifcontinued(status: i32) -> bool {
    (status & 0xffff) == 0xffff
}
