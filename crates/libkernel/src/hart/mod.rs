use alloc::vec::Vec;
use core::{
    arch::asm,
    cell::{Cell, Ref, RefCell, SyncUnsafeCell},
    ptr::NonNull,
};

use common::config::MAX_HART_NUM;
use crossbeam_utils::CachePadded;
use kernel_tracer::SpanId;
use triomphe::Arc;

use crate::{process::Process, thread::Thread, trap::TrapContext};

// `CachePadded` 可以保证 per-cpu 的结构位于不同的 cache line 中
// 因此避免 false sharing
static HARTS: [SyncUnsafeCell<CachePadded<Hart>>; MAX_HART_NUM] =
    [const { SyncUnsafeCell::new(CachePadded::new(Hart::new())) }; MAX_HART_NUM];

/// # SAFETY
/// Hart 结构实际上只会被对应的 hart 访问
unsafe impl Sync for Hart {}

/// 可以认为代表一个处理器。存放一些 per-hart 的数据
///
/// 因此，一般可以假定不会被并行访问
pub struct Hart {
    hart_id: usize,
    // TODO: 内核线程是不是会不太一样？
    /// 当前 hart 上正在运行的线程。
    thread: RefCell<Option<Arc<Thread>>>,
    pub span_stack: RefCell<Vec<SpanId>>,
    pub panicked: Cell<bool>,
}

impl Hart {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Hart {
        Hart {
            hart_id: 0,
            thread: RefCell::new(None),
            span_stack: RefCell::new(Vec::new()),
            panicked: Cell::new(false),
        }
    }

    pub fn hart_id(&self) -> usize {
        self.hart_id
    }

    // TODO: [low] 或许可以通过使 `replace_thread()` unsafe 来避免 `RefCell` 的开销
    pub fn replace_thread(&self, new_thread: Option<Arc<Thread>>) -> Option<Arc<Thread>> {
        core::mem::replace(&mut self.thread.borrow_mut(), new_thread)
    }

    pub fn curr_thread(&self) -> Ref<'_, Thread> {
        Ref::map(self.thread.borrow(), |t| t.as_ref().unwrap().as_ref())
    }

    /// 辅助方法，相当于 `curr_thread()` 并从中取出 trap context
    pub fn curr_trap_context(&self) -> NonNull<TrapContext> {
        NonNull::from(unsafe { &mut self.curr_thread().get_owned().as_mut().trap_context })
    }

    pub fn curr_process(&self) -> Ref<'_, Process> {
        Ref::map(self.curr_thread(), |t| t.process.as_ref())
    }

    pub fn curr_process_arc(&self) -> Ref<'_, Arc<Process>> {
        Ref::map(self.curr_thread(), |t| &t.process)
    }
}

/// 设置当前 hart 的 `Hart` 结构，将 `tp` 设置为其地址
///
/// # Safety
///
/// 需保证由不同 hart 调用
pub unsafe fn set_local_hart(hart_id: usize) {
    unsafe {
        let hart_ptr = HARTS[hart_id].get();
        (&mut (*hart_ptr)).hart_id = hart_id;
        asm!("mv tp, {}", in(reg) hart_ptr as usize);
    }
}

pub fn local_hart<'a>() -> &'a Hart {
    let tp: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) tp);
        &*(tp as *const Hart)
    }
}
