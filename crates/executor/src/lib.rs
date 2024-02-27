#![no_std]

mod yield_now;

#[macro_use]
extern crate kernel_tracer;

use core::future::Future;

use async_task::{Runnable, Task};
use defines::config::TASK_LIMIT;
use heapless::mpmc::MpMcQueue;
use klocks::Lazy;

pub use self::yield_now::yield_now;

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

pub fn spawn<F>(future: F) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    async_task::spawn(future, |runnable| {
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
