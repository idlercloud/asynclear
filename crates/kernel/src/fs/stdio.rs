use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use defines::error::KResult;
use user_check::{UserCheck, UserCheckMut};

use crate::{drivers::qemu_uart::TTY, thread::BlockingFuture, uart_console::print};

pub async fn read_stdin(buf: UserCheckMut<[u8]>) -> KResult<usize> {
    BlockingFuture::new(TtyFuture::new(buf)).await
}

pub fn write_stdout(buf: UserCheck<[u8]>) -> KResult<usize> {
    let buf = buf.check_slice()?;
    let s = core::str::from_utf8(&buf).unwrap();
    print!("{s}");
    Ok(buf.len())
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TtyFuture {
    user_buf: UserCheckMut<[u8]>,
}

impl TtyFuture {
    pub fn new(user_buf: UserCheckMut<[u8]>) -> Self {
        Self { user_buf }
    }
}

impl Future for TtyFuture {
    type Output = KResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut tty = TTY.lock();
        let mut cnt = 0;
        let mut user_buf = self.user_buf.check_slice_mut()?;
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
