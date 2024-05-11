use defines::signal::KSignalSet;

use crate::trap::TrapContext;

pub struct ThreadInner {
    pub trap_context: TrapContext,

    // TODO: [blocked] thread。实现 clear_child_tid。<https://man7.org/linux/man-pages/man2/set_tid_address.2.html>
    #[allow(unused)]
    pub clear_child_tid: usize,

    // 信号
    pub signal_mask: KSignalSet,
    pub pending_signal: KSignalSet,
}
