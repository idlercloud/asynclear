use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// 即刻让出控制权，并且立刻 wake（一般而言就是立刻重新进入就绪队列）
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn yield_now() -> impl Future<Output = ()> {
    YieldFuture { first_pool: true }
}

struct YieldFuture {
    first_pool: bool,
}

impl Future for YieldFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.first_pool {
            self.first_pool = false;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
