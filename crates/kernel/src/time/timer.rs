use alloc::collections::BinaryHeap;
use core::{
    cmp::{Ordering, Reverse},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};

use klocks::SpinNoIrqMutex;

struct TimerFuture {
    expire_ms: usize,
    timer_activated: bool,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.expire_ms > riscv_time::get_time_ms() {
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

struct Timer {
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
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.expire_ms.cmp(&other.expire_ms)
    }
}

static TIMERS: SpinNoIrqMutex<BinaryHeap<Reverse<Timer>>> =
    SpinNoIrqMutex::new(BinaryHeap::<Reverse<Timer>>::new());

pub fn check_timer() {
    let mut timers = TIMERS.lock();
    let curr_ms = riscv_time::get_time_ms();
    while let Some(timer) = timers.peek() {
        if curr_ms >= timer.0.expire_ms {
            let timer = timers.pop().unwrap();
            timer.0.waker.wake();
        } else {
            break;
        }
    }
}

pub fn sleep(time: Duration) -> impl Future<Output = ()> {
    let curr_ms = riscv_time::get_time_ms();
    let expire_ms = curr_ms + time.as_millis() as usize;
    TimerFuture {
        expire_ms,
        timer_activated: false,
    }
}
