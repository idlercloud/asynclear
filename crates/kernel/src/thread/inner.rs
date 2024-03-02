use defines::trap_context::TrapContext;

pub struct ThreadInner {
    /// 线程的退出码，在 `sys_exit` 时被设置。
    ///
    /// 如果它是进程中的最后一个线程，则将进程退出码设置为它。
    pub exit_code: i8,
    pub thread_status: ThreadStatus,
    pub trap_context: TrapContext,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ThreadStatus {
    Ready,
    Running,
    // Sleeping,
    Terminated,
}
