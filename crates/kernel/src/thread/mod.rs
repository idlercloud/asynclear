mod inner;
mod user;

use atomic::{Atomic, Ordering};
use common::config::{LOW_ADDRESS_END, PAGE_SIZE, USER_STACK_SIZE};
use defines::signal::KSignalSet;
use klocks::{SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use self::inner::ThreadInner;
pub use self::user::{spawn_user_thread, BlockingFuture};
use crate::{
    memory::{AreaType, MapPermission, MemorySpace, VirtAddr, VirtPageNum},
    process::Process,
    trap::TrapContext,
};

/// 进程控制块
pub struct Thread {
    tid: usize,
    pub status: Atomic<ThreadStatus>,
    /// 线程的退出码，在 `sys_exit` 时被设置。
    ///
    /// 如果它是进程中的最后一个线程，则将进程退出码设置为它。
    pub exit_code: Atomic<i8>,
    pub process: Arc<Process>,
    inner: SpinMutex<ThreadInner>,
}

impl Thread {
    pub fn new(
        process: Arc<Process>,
        tid: usize,
        trap_context: TrapContext,
        signal_mask: KSignalSet,
    ) -> Self {
        Self {
            tid,
            exit_code: Atomic::new(0),
            status: Atomic::new(ThreadStatus::Ready),
            process,
            inner: SpinMutex::new(ThreadInner {
                trap_context,
                clear_child_tid: 0,
                signal_mask,
                pending_signal: KSignalSet::empty(),
            }),
        }
    }

    pub fn tid(&self) -> usize {
        self.tid
    }

    pub fn lock_inner(&self) -> SpinMutexGuard<'_, ThreadInner> {
        self.inner.lock()
    }

    /// 锁 inner 然后进行操作，这是一个便捷方法
    pub fn lock_inner_with<T>(&self, f: impl FnOnce(&mut ThreadInner) -> T) -> T {
        f(&mut self.inner.lock())
    }

    /// 分配用户栈，一般用于创建新线程。返回用户栈高地址
    ///
    /// 注意 `memory_space` 是本进程的 `MemorySpace`
    pub fn alloc_user_stack(tid: usize, memory_space: &mut MemorySpace) -> VirtPageNum {
        // 分配用户栈
        let ustack_low_vpn = Self::user_stack_low_addr(tid);
        let ustack_high_vpn = Self::user_stack_high_addr(tid);
        let ustack_low_addr = ustack_low_vpn.page_start();
        let ustack_high_addr = ustack_high_vpn.page_start();
        trace!(
            "user stack is {:#x}..{:#x}",
            ustack_low_addr.0,
            ustack_high_addr.0
        );

        // 栈地址都是根据 tid 确定的，不会冲突
        unsafe {
            memory_space.user_map(
                ustack_low_addr,
                ustack_high_addr,
                MapPermission::R | MapPermission::W | MapPermission::U,
                AreaType::Lazy,
            );
        }
        ustack_high_vpn
    }

    /// 获取当前线程用户栈的低地址，即高地址减去用户栈大小
    fn user_stack_low_addr(tid: usize) -> VirtPageNum {
        Self::user_stack_high_addr(tid) - VirtAddr(USER_STACK_SIZE).vpn_floor().0
    }

    /// 获取当前线程用户栈的高地址
    fn user_stack_high_addr(tid: usize) -> VirtPageNum {
        // 注意每个用户栈后都会有一个 Guard Page
        VirtAddr(LOW_ADDRESS_END - tid * (USER_STACK_SIZE + PAGE_SIZE)).vpn_floor()
    }

    /// 释放用户栈。一般是单个线程退出时使用。
    ///
    /// 注意 `memory_space` 是本进程的 `MemorySpace`
    fn dealloc_user_stack(&self, memory_space: &mut MemorySpace) {
        // 手动取消用户栈的映射
        let user_stack_low_addr = Self::user_stack_low_addr(self.tid);
        memory_space.remove_area_with_start_vpn(user_stack_low_addr);
    }

    pub fn set_status(&self, status: ThreadStatus) {
        self.status.store(status, Ordering::SeqCst);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ThreadStatus {
    Ready,
    Running,
    Blocking,
    Terminated,
}

unsafe impl bytemuck::NoUninit for ThreadStatus {}

const _: () = assert!(USER_STACK_SIZE % PAGE_SIZE == 0 && LOW_ADDRESS_END % PAGE_SIZE == 0);
