use core::{
    future::Future,
    mem,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use hashbrown::HashMap;
use riscv::register::sstatus::FS;
use triomphe::Arc;

use super::Thread;
use crate::{
    executor,
    fs::VFS,
    hart::local_hart,
    memory::KERNEL_SPACE,
    process::{ProcessStatus, INITPROC},
    thread::ThreadStatus,
    SHUTDOWN,
};

pub fn spawn_user_thread(thread: Arc<Thread>) {
    let (runnable, task) = executor::spawn_with(
        UserThreadWrapperFuture::new(Arc::clone(&thread), user_thread_loop::user_thread_loop()),
        move || thread.set_status(ThreadStatus::Ready),
    );
    runnable.schedule();
    task.detach();
}

mod user_thread_loop {
    use futures::Future;

    use crate::{hart::local_hart, trap};

    pub type UserThreadFuture = impl Future<Output = ()> + Send;

    pub fn user_thread_loop() -> UserThreadFuture {
        async {
            loop {
                // 返回用户态
                // 注意切换了控制流，但是之后回到内核态还是在这里
                trace!("enter user mode");
                trap::trap_return(local_hart().curr_trap_context());
                trace!("enter kernel mode");

                // 在内核态处理 trap。注意这里也可能切换控制流，让出 Hart 给其他线程
                let next_op = trap::user_trap_handler().await;

                if next_op.is_break() || local_hart().curr_process().is_exited() {
                    break;
                }
            }
        }
    }
}

fn exit_thread(thread: &Thread) {
    debug!("thread exits");
    let process = &thread.process;
    let mut process_inner = process.lock_inner();
    process_inner
        .threads
        .remove(&thread.tid)
        .expect("remove thread here");
    process_inner.tid_allocator.dealloc(thread.tid);
    thread.dealloc_user_stack(&mut process_inner.memory_space);
    thread.set_status(ThreadStatus::Terminated);

    // 如果是最后一个线程，则该进程成为僵尸进程，等待父进程 wait
    // 如果父进程不 wait 的话，就一直存活着，并占用 pid 等资源
    // 但主要的资源是会释放的，比如地址空间、线程控制块等
    if process_inner.threads.is_empty() {
        info!("all threads exit");
        // 不太想让 `cwd` 加个 `Option`，但是也最好不要保持原来的引用了，所以引到根目录去得了
        process_inner.cwd = Arc::clone(VFS.root_dir());
        process_inner.memory_space.recycle_user_pages();
        process_inner.threads = HashMap::new();
        process_inner.tid_allocator.release();
        let children = mem::take(&mut process_inner.children);
        let parent = process_inner.parent.take();
        drop(process_inner);

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

        // 子进程交由 INITPROC 来处理。如果退出的就是 INITPROC，那么系统退出
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

/// `UserThreadWrapperFuture` 用来处理用户线程获取控制权以及让出控制权时的上下文切换。如页表切换等
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project]
struct UserThreadWrapperFuture {
    #[pin]
    future: user_thread_loop::UserThreadFuture,
    thread: Arc<Thread>,
}

impl UserThreadWrapperFuture {
    #[inline]
    fn new(thread: Arc<Thread>, future: user_thread_loop::UserThreadFuture) -> Self {
        Self { thread, future }
    }
}

impl Future for UserThreadWrapperFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        local_hart().replace_thread(Some(Arc::clone(&self.thread)));
        let process = &self.thread.process;
        process.lock_inner_with(|inner| inner.memory_space.activate());
        let pid = process.pid();
        let tid = self.thread.tid;
        let _enter = info_span!("task", pid = pid, tid = tid).entered();
        let trap_context = unsafe { &mut self.thread.get_owned().as_mut().trap_context };
        if trap_context.user_float_ctx.valid {
            trap_context.user_float_ctx.restore();
        }
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
        } else {
            if project.thread.status.load(Ordering::SeqCst) != ThreadStatus::Ready {
                project.thread.set_status(ThreadStatus::Blocking);
            }
            let trap_context = unsafe { &mut project.thread.get_owned().as_mut().trap_context };
            if trap_context.fs() == FS::Dirty {
                // 进入信号处理前需要保存当前线程的浮点数上下文以便信号处理完成后恢复
                trap_context.user_float_ctx.save();
                trap_context.user_float_ctx.valid = true;
                trap_context.set_fs(FS::Clean);
            }
        }

        // NOTE: 一定要切换页表。否则进程页表被回收立刻导致内核异常
        // 但可以不刷新 tlb。因为内核中只会用到共享的、永远映射的内核高地址空间
        unsafe {
            KERNEL_SPACE.activate_no_tlb();
        }
        trace!("User task deactivate");
        local_hart().replace_thread(None);

        ret
    }
}
