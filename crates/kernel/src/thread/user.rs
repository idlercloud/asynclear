use core::{
    future::Future,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use alloc::collections::BTreeMap;
use compact_str::CompactString;
use triomphe::Arc;

use crate::{
    executor,
    hart::{local_hart, local_hart_mut},
    process::{ProcessStatus, INITPROC},
    thread::ThreadStatus,
    trap, SHUTDOWN,
};

use super::Thread;

pub fn spawn_user_thread(thread: Arc<Thread>) {
    let (runnable, task) = executor::spawn_with(
        UserThreadFuture::new(Arc::clone(&thread), user_thread_loop()),
        move || thread.set_status(ThreadStatus::Ready),
    );
    runnable.schedule();
    task.detach();
}

async fn user_thread_loop() {
    loop {
        // 返回用户态
        // 注意切换了控制流，但是之后回到内核态还是在这里
        trap::trap_return(unsafe {
            (*local_hart())
                .curr_thread()
                .lock_inner_with(|inner| &mut inner.trap_context as _)
        });

        trace!("enter kernel mode");
        // 在内核态处理 trap。注意这里也可能切换控制流，让出 Hart 给其他线程
        let next_op = trap::user_trap_handler().await;

        if next_op.is_break() || unsafe { (*local_hart()).curr_process().is_exited() } {
            break;
        }
    }
}

// 这里对线程的引用应该是最后几个了，剩下应该只在 Hart 相关的结构中存有
fn exit_thread(thread: &Thread) {
    debug!("thread exits");
    let process = &thread.process;
    let children = process.lock_inner_with(|process_inner| {
        process_inner.threads.remove(&thread.tid);
        process_inner.tid_allocator.dealloc(thread.tid);
        thread.dealloc_user_stack(&mut process_inner.memory_set);
        thread.set_status(ThreadStatus::Terminated);

        // 如果是最后一个线程，则该进程成为僵尸进程，等待父进程 wait
        // 如果父进程不 wait 的话，就一直存活着，并占用 pid 等资源
        // 但主要的资源是会释放的，比如地址空间、线程控制块等
        if process_inner.threads.is_empty() {
            info!("all threads exit");
            process_inner.cwd = CompactString::new("");
            // 根页表以及内核相关的部分要留着
            process_inner.memory_set.recycle_user_pages();
            process_inner.threads = BTreeMap::new();
            process_inner.tid_allocator.release();

            // 如果进程已标记为退出（即已调用 `exit_process()`），则标记为僵尸并使用已有的退出码
            // 否则使用线程的退出码
            let exit_code = thread.exit_code.load(Ordering::SeqCst);
            let new_status;
            if let Some(override_exit_code) = process.exit_code() {
                new_status = ProcessStatus::zombie(override_exit_code);
            } else {
                new_status = ProcessStatus::zombie(exit_code);
            }
            process.status.store(new_status, Ordering::SeqCst);

            // 通知父进程自己退出了
            if let Some(parent) = process_inner.parent.take() {
                if let Some(exit_signal) = process.exit_signal {
                    todo!("[high] add exit_signal support")
                }

                parent.wait4_event.notify(1);
            }
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
                        child_inner.parent = Some(Arc::clone(&INITPROC));
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
        let process = &self.thread.process;
        process.lock_inner_with(|inner| inner.memory_set.activate());
        let pid = process.pid();
        let tid = self.thread.tid;
        let _enter = info_span!("task", pid = pid, tid = tid).entered();
        trace!("User task running");
        let prev_status = self
            .thread
            .status
            .swap(ThreadStatus::Running, Ordering::SeqCst);
        if prev_status != ThreadStatus::Ready {
            panic!("Run unready({prev_status:?}) task")
        }

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

/// 在 Pending 时会将线程标记为 `Blocking` 的 Future
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project]
pub struct BlockingFuture<F> {
    #[pin]
    future: F,
}

impl<F> BlockingFuture<F> {
    pub fn new(future: F) -> Self {
        Self { future }
    }
}

impl<F: Future> Future for BlockingFuture<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pinned = self.project().future;
        let ret = pinned.poll(cx);
        if ret.is_pending() {
            unsafe {
                (*local_hart())
                    .curr_thread()
                    .set_status(ThreadStatus::Blocking);
            }
        }
        ret
    }
}
