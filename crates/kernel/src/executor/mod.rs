mod yield_now;

pub use yield_now::yield_now;

use core::future::Future;

use async_task::{Runnable, Task};
use common::config::TASK_LIMIT;
use heapless::mpmc::MpMcQueue;
use klocks::Lazy;

static TASK_QUEUE: Lazy<TaskQueue> = Lazy::new(TaskQueue::new);

/// NOTE: 目前的实现中，并发的任务量是有硬上限 (`TASK_LIMIT`) 的，超过会直接 panic
struct TaskQueue {
    queue: MpMcQueue<Runnable, TASK_LIMIT>,
}

impl TaskQueue {
    fn new() -> Self {
        Self {
            queue: MpMcQueue::new(),
        }
    }

    fn push_task(&self, runnable: Runnable) {
        self.queue.enqueue(runnable).expect("Out of task limit");
    }

    fn fetch_task(&self) -> Option<Runnable> {
        self.queue.dequeue()
    }
}

// pub fn spawn<F>(future: F) -> (Runnable, Task<F::Output>)
// where
//     F: Future + Send + 'static,
//     F::Output: Send + 'static,
// {
//     async_task::spawn(future, |runnable| {
//         TASK_QUEUE.push_task(runnable);
//     })
// }

pub fn spawn_with<F, A>(future: F, action: A) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
    A: Fn() + Send + Sync + 'static,
{
    // TODO: 现在这么操作用于让用户线程被调度时状态设为 `Ready`，其实可能可以有更好的方式
    async_task::spawn(future, move |runnable| {
        action();
        TASK_QUEUE.push_task(runnable);
    })
}

pub fn run_utils_idle(should_shutdown: fn() -> bool) {
    loop {
        while let Some(task) = TASK_QUEUE.fetch_task() {
            trace!("Schedule new task");
            task.run();
        }
        if should_shutdown() {
            break;
        }
        sbi_rt::hart_suspend(sbi_rt::Retentive, 0, 0);
    }
}
