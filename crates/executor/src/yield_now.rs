use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// 即刻让出控制权，并且立刻 wake（一般而言就是立刻重新进入就绪队列）
pub fn yield_now() -> impl Future<Output = ()> {
    YieldFuture(false)
}

struct YieldFuture(bool);

impl Future for YieldFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        // Wake up this future, which means putting this thread into
        // the tail of the task queue
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
