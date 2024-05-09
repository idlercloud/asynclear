use defines::signal::KSignalSet;

use crate::trap::TrapContext;

pub struct ThreadInner {
    pub trap_context: TrapContext,

    // TODO: [blocked] thread。实现 clear_child_tid。https://man7.org/linux/man-pages/man2/set_tid_address.2.html
    #[allow(unused)]
    pub clear_child_tid: usize,

    // 信号
    /// 虽然这里应该是个 [`SignalSet`]，但实际上只有第一个 `SignalFlag` 被使用
    ///
    /// [`SignalSet`]: signal::SignalSet
    pub signal_mask: KSignalSet,
    pub pending_signal: KSignalSet,
}
