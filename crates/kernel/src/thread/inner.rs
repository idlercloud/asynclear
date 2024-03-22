use defines::signal::KSignalSet;

use crate::trap::TrapContext;

pub struct ThreadInner {
    pub trap_context: TrapContext,

    /* 信号 */
    /// 虽然这里应该是个 [`SignalSet`]，但实际上只有第一个 `SignalFlag` 被使用
    ///
    /// [`SignalSet`]: signal::SignalSet
    pub signal_mask: KSignalSet,
    pub pending_signal: KSignalSet,
}
