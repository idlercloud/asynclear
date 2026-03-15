mod inner;

use core::{cell::SyncUnsafeCell, mem, ops::Range, ptr::NonNull};

use atomic::{Atomic, Ordering};
use common::config::{LOW_ADDRESS_END, PAGE_SIZE, USER_STACK_SIZE};
use hashbrown::HashMap;
use klocks::{SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use self::inner::{ThreadInner, ThreadOwned};
use crate::{
    fs::VirtFileSystem,
    memory::{self, MapPermission, MemorySpace, VirtAddr, VirtPageNum},
    process::{self, Process, ProcessStatus},
    signal::KSignalSet,
    trap::TrapContext,
};

/// 进程控制块
pub struct Thread {
    tid: usize,
    /// 线程状态
    pub status: Atomic<ThreadStatus>,
    /// 线程的退出码，在 `sys_exit` 时被设置。
    ///
    /// 如果它是进程中的最后一个线程，则将进程退出码设置为它。
    pub exit_code: Atomic<i8>,
    /// 所属的进程
    pub process: Arc<Process>,
    /// 可能被并发访问的可变结构
    inner: SpinMutex<ThreadInner>,
    /// 不应被并发访问的可变结构
    owned: SyncUnsafeCell<ThreadOwned>,
}

impl Thread {
    pub fn new(process: Arc<Process>, tid: usize, trap_context: TrapContext, signal_mask: KSignalSet) -> Self {
        Self {
            tid,
            exit_code: Atomic::new(0),
            status: Atomic::new(ThreadStatus::Ready),
            process,
            inner: SpinMutex::new(ThreadInner {
                signal_mask,
                pending_signal: KSignalSet::empty(),
            }),
            owned: SyncUnsafeCell::new(ThreadOwned {
                trap_context,
                clear_child_tid: 0,
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

    /// 获取线程私有的值，只应由当前运行该线程的 hart 访问
    pub fn get_owned(&self) -> NonNull<ThreadOwned> {
        unsafe { NonNull::new_unchecked(self.owned.get()) }
    }

    /// 分配用户栈，一般用于创建新线程。返回用户栈范围
    ///
    /// 注意 `memory_space` 是本进程的 `MemorySpace`
    pub fn alloc_user_stack(tid: usize, memory_space: &mut MemorySpace) -> Range<VirtPageNum> {
        // 分配用户栈
        let ustack_low_vpn = Self::user_stack_low_addr(tid);
        let ustack_high_vpn = Self::user_stack_high_addr(tid);
        trace!(
            "user stack is {:#x}..{:#x}",
            ustack_low_vpn.page_start().0,
            ustack_high_vpn.page_start().0
        );

        // 栈地址都是根据 tid 确定的，不会冲突
        unsafe {
            memory_space.user_map(
                ustack_low_vpn..ustack_high_vpn,
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
        }
        ustack_low_vpn..ustack_high_vpn
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
        memory::flush_tlb(None);
    }

    pub fn set_status(&self, status: ThreadStatus) {
        self.status.store(status, Ordering::SeqCst);
    }

    pub fn exit_thread(&self) {
        debug!("thread exits");
        let process = &self.process;
        let mut process_inner = process.lock_inner();
        process_inner.threads.remove(&self.tid).expect("remove thread here");
        process_inner.tid_allocator.dealloc(self.tid);
        self.dealloc_user_stack(&mut process_inner.memory_space);
        self.set_status(ThreadStatus::Terminated);

        // 如果是最后一个线程，则该进程成为僵尸进程，等待父进程 wait
        // 如果父进程不 wait 的话，就一直存活着，并占用 pid 等资源
        // 但主要的资源是会释放的，比如地址空间、线程控制块等
        if process_inner.threads.is_empty() {
            info!("all threads exit");
            // 不太想让 `cwd` 加个 `Option`，但是也最好不要保持原来的引用了，所以引到根目录去得了
            process_inner.cwd = Arc::clone(VirtFileSystem::instance().root_dir());
            process_inner.memory_space.recycle_user_pages();
            process_inner.threads = HashMap::new();
            process_inner.tid_allocator.release();
            let children = mem::take(&mut process_inner.children);
            let parent = process_inner.parent.take();
            drop(process_inner);

            // 如果进程已标记为退出（即已调用 `exit_process()`），则标记为僵尸并使用已有的退出码
            // 否则使用线程的退出码
            let exit_code = self.exit_code.load(Ordering::SeqCst);
            let new_status;
            if let Some(override_exit_code) = process.exit_code() {
                new_status = ProcessStatus::zombie(override_exit_code);
            } else {
                new_status = ProcessStatus::zombie(exit_code);
            }
            process.status.store(new_status, Ordering::SeqCst);

            // 子进程交由 INITPROC 来处理。如果退出的就是 INITPROC，那么系统退出
            if process.pid() == process::INITPROC_PID {
                assert_eq!(children.len(), 0);
                executor::SHUTDOWN.store(true, Ordering::SeqCst);
            } else {
                let init_proc = process::PROCESS_MANAGER.init_proc();
                init_proc.lock_inner_with(|initproc_inner| {
                    for child in children {
                        child.lock_inner_with(|child_inner| {
                            child_inner.parent = Some(Arc::clone(&init_proc));
                        });
                        initproc_inner.children.push(child);
                    }
                });
                // FIXME: 应该需要通知 INITPROC 的，否则有可能子进程已经是僵尸了而 INITPROC 不知道
                // 下面这个应该够了？但是不完全确定，需要仔细思考各种并发的情况
                // INITPROC.wait4_event.notify(1);
            }

            // 通知父进程自己退出了
            if let Some(parent) = parent {
                if let Some(exit_signal) = process.exit_signal {
                    parent.lock_inner_with(|inner| inner.receive_signal(exit_signal));
                }

                parent.wait4_event.notify(1);
            }
        }
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

const _: () = assert!(USER_STACK_SIZE.is_multiple_of(PAGE_SIZE) && LOW_ADDRESS_END.is_multiple_of(PAGE_SIZE));
