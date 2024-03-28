mod stdio;

use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use triomphe::Arc;

use async_trait::async_trait;
use defines::error::Result;
use user_check::UserCheckMut;

use crate::drivers::qemu_uart::TTY;

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
    type Output = Result<usize>;
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

#[derive(Clone)]
pub enum File {
    Stdin,
    Stdout,
    DynFile(Arc<dyn DynFile>),
}

impl File {
    pub async fn read(&self, buf: UserCheckMut<u8>) -> Result<usize> {
        match self {
            File::Stdin => todo!(),
            File::Stdout => todo!(),
            File::DynFile(_) => todo!(),
        }
    }
}

#[async_trait]
pub trait DynFile {
    async fn read(&self, buf: &mut [u8]) -> Result<usize>;
    async fn write(&self, buf: &[u8]) -> Result<usize>;
}
