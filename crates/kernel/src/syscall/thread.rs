use defines::error::Result;

use crate::hart::local_hart;

/// 获取线程 tid。永远成功
///
/// TODO: Linux 手册里说，单线程进程中返回 pid，而多线程进程返回 tid。这里一直返回 tid（而且 tid 的定义可能也不同）
///
/// <https://man7.org/linux/man-pages/man2/gettid.2.html>
pub fn sys_gettid() -> Result {
    Ok(unsafe { (*local_hart()).curr_thread().tid() } as isize)
}
