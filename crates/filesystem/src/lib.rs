#![no_std]

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use defines::error::Result;
use qemu_uart::TTY;
use user_check::UserCheck;

pub struct TtyFuture {
    user_buf: UserCheck<u8>,
    len: usize,
}

impl TtyFuture {
    pub fn new(user_buf: UserCheck<u8>, len: usize) -> Self {
        Self { user_buf, len }
    }
}

impl Future for TtyFuture {
    type Output = Result<usize>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut tty = TTY.lock();
        let mut cnt = 0;
        let mut user_buf = self.user_buf.check_slice_mut(self.len)?;
        loop {
            if cnt >= user_buf.len() {
                break;
            }
            if let Some(byte) = tty.get_byte() {
                user_buf[cnt] = byte;
                cnt += 1;
            } else {
                break;
            }
        }
        if cnt > 0 {
            Poll::Ready(Ok(cnt))
        } else {
            tty.register_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}
