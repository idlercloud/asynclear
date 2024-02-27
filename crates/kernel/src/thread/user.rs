use core::{
    future::Future,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use compact_str::CompactString;

use crate::{
    hart::{curr_process, local_hart, local_hart_mut},
    process::INITPROC,
    trap, SHUTDOWN,
};

use super::{inner::ThreadStatus, Thread};

pub fn spawn_user_thread(thread: Arc<Thread>) {
    let (runnable, task) = executor::spawn(UserThreadFuture::new(
        Arc::clone(&thread),
        user_thread_loop(),
    ));
    runnable.schedule();
    task.detach();
}

async fn user_thread_loop() {
    loop {
        // 返回用户态
        // 注意切换了控制流，但是之后回到内核态还是在这里
        trap::trap_return(unsafe { (*local_hart()).trap_context() });

        trace!("enter kernel mode");
        // 在内核态处理 trap。注意这里也可能切换控制流，让出 Hart 给其他线程
        let next_op = trap::user_trap_handler().await;

        if next_op.is_break() || curr_process().lock_inner_with(|inner| inner.exit_code.is_some()) {
            break;
        }
    }
}

// 这里对线程的引用应该是最后几个了，剩下应该只在 Hart 相关的结构中存有
fn exit_thread(thread: &Thread) {
    let process = thread.process.upgrade().unwrap();

    debug!("one thread exits");
    let children = process.lock_inner_with(|process_inner| {
        process_inner.threads.remove(&thread.tid);
        process_inner.tid_allocator.dealloc(thread.tid);
        thread.dealloc_user_stack(&mut process_inner.memory_set);
        let exit_code = thread.lock_inner(|thread_inner| {
            thread_inner.thread_status = ThreadStatus::Terminated;
            thread_inner.exit_code
        });

        // 如果是最后一个线程，则该进程成为僵尸进程，等待父进程 wait
        // 如果父进程不 wait 的话，就一直存活着，并占用 pid 等资源
        // 但主要的资源是会释放的，比如地址空间、线程控制块等
        if process_inner.threads.is_empty() {
            info!("all threads exit");
            // 如果进程尚未被标记为僵尸，则将线程的退出码赋予给它
            if process_inner.exit_code.is_none() {
                process_inner.exit_code = Some(exit_code);
            }
            process_inner.cwd = CompactString::new("");
            // 根页表以及内核相关的部分要留着
            process_inner.memory_set.recycle_user_pages();
            process_inner.threads = BTreeMap::new();
            process_inner.tid_allocator.release();

            // 通知父进程自己退出了
            if let Some(parent) = process_inner.parent.upgrade() {
                parent.wait4_event.notify(1);
            }
            process_inner.parent = Weak::new();
            Some(core::mem::take(&mut process_inner.children))
        } else {
            None
        }
    });
    // 子进程交由 INITPROC 来处理。如果退出的就是 INITPROC，那么系统退出
    if let Some(children) = children {
        if process.pid() == 1 {
            assert_eq!(children.len(), 0);
            SHUTDOWN.store(true, Ordering::SeqCst);
        } else {
            INITPROC.lock_inner_with(|initproc_inner| {
                for child in children {
                    child.lock_inner_with(|child_inner| {
                        child_inner.parent = Arc::downgrade(&INITPROC)
                    });
                    initproc_inner.children.push(child);
                }
            });
        }
    }
}

/// `UserThreadFuture` 用来处理用户线程获取控制权以及让出控制权时的上下文切换。如页表切换等
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project]
struct UserThreadFuture<F: Future + Send> {
    #[pin]
    future: F,
    thread: Arc<Thread>,
}

impl<F: Future + Send> UserThreadFuture<F> {
    #[inline]
    fn new(thread: Arc<Thread>, future: F) -> Self {
        Self { thread, future }
    }
}

impl<F: Future + Send> Future for UserThreadFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            (*local_hart_mut()).replace_thread(Some(Arc::clone(&self.thread)));
        }
        let process = self.thread.process.upgrade().unwrap();
        process.lock_inner_with(|inner| inner.memory_set.activate());
        let pid = process.pid();
        let tid = self.thread.tid;
        let _enter = info_span!("task", pid = pid, tid = tid).entered();
        trace!("User task running");
        self.thread.lock_inner(|inner| {
            inner.thread_status = ThreadStatus::Running;
        });

        let project = self.project();
        let ret = project.future.poll(cx);

        if ret.is_ready() {
            exit_thread(project.thread);
        }

        // 该进程退出运行态。不过页表不会切换
        // 进程状态的切换由 `user_thread_loop()` 里的操作完成
        trace!("User task deactivate");
        unsafe {
            (*local_hart_mut()).replace_thread(None);
        }

        ret
    }
}
