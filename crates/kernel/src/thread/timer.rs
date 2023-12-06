use core::{
    cmp::{Ordering, Reverse},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use alloc::collections::BinaryHeap;
use spin::Mutex;

pub struct TimerFuture {
    expire_ms: usize,
    timer_activated: bool,
}

impl Future for TimerFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.expire_ms < riscv_time::get_time_ms() {
            if !self.timer_activated {
                TIMERS.lock().push(Reverse(Timer {
                    expire_ms: self.expire_ms,
                    waker: cx.waker().clone(),
                }));
                self.timer_activated = true;
            }
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub struct Timer {
    expire_ms: usize,
    waker: Waker,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.expire_ms.partial_cmp(&other.expire_ms)
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.expire_ms.cmp(&other.expire_ms)
    }
}

static TIMERS: Mutex<BinaryHeap<Reverse<Timer>>> = Mutex::new(BinaryHeap::<Reverse<Timer>>::new());

/// 返回值表示在初赛测试中是否可以继续而非等待
pub fn check_timer() {
    let current_ms = riscv_time::get_time_ms();
    let mut timers = TIMERS.lock();
    while let Some(timer) = timers.peek() {
        if current_ms >= timer.0.expire_ms {
            let timer = timers.pop().unwrap();
            timer.0.waker.wake();
        } else {
            break;
        }
    }
}
