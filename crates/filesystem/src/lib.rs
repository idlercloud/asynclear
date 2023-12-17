#![no_std]

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use defines::user_ptr::UserMut;
use qemu_uart::TTY;

pub struct TtyFuture {
    user_buf: UserMut<[u8]>,
}

impl TtyFuture {
    pub fn new(user_buf: UserMut<[u8]>) -> Self {
        Self { user_buf }
    }
}

impl Future for TtyFuture {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut tty = TTY.lock();
        let mut cnt = 0;
        let user_buf_len = unsafe { (*self.user_buf.raw()).len() };
        loop {
            if cnt >= user_buf_len {
                break;
            }
            if let Some(byte) = tty.get_byte() {
                // FIXME: 如果引入换页，这里是有问题的
                unsafe {
                    (*self.user_buf.raw())[cnt] = byte;
                }
                cnt += 1;
            } else {
                break;
            }
        }
        if cnt > 0 {
            Poll::Ready(cnt)
        } else {
            tty.register_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}
