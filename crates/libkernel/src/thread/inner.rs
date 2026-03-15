use crate::{signal::KSignalSet, trap::TrapContext};

pub struct ThreadInner {
    // 信号
    /// 信号掩码
    pub signal_mask: KSignalSet,
    /// 待处理信号队列
    pub pending_signal: KSignalSet,
}

/// 线程拥有的值，只会由线程自己访问的值，因此可以包裹在 [`UnsafeCell`] 中
///
/// [`UnsafeCell`]: core::cell::UnsafeCell
pub struct ThreadOwned {
    /// 陷入上下文
    pub trap_context: TrapContext,

    // TODO: [blocked] thread。实现 clear_child_tid。<https://man7.org/linux/man-pages/man2/set_tid_address.2.html>
    #[allow(unused)]
    pub clear_child_tid: usize,
}
