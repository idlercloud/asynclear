use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use compact_str::CompactString;
use defines::{
    error::KResult,
    ioctl::{
        WinSize, TCGETA, TCGETS, TCSBRK, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGWINSZ,
        TIOCSPGRP, TIOCSWINSZ,
    },
};
use klocks::{Lazy, SpinMutex};

use super::inode::{Inode, InodeMeta, InodeMode};
use crate::{
    drivers::qemu_uart::TTY, memory::UserCheck, process::INITPROC, thread::BlockingFuture,
    uart_console::print,
};

pub async fn read_stdin(buf: UserCheck<[u8]>) -> KResult<usize> {
    BlockingFuture::new(TtyFuture::new(buf)).await
}

pub fn write_stdout(buf: UserCheck<[u8]>) -> KResult<usize> {
    let buf = buf.check_slice()?;
    let s = core::str::from_utf8(&buf).unwrap();
    print!("{s}");
    Ok(buf.len())
}

pub fn get_tty_inode() -> &'static Inode<TtyInode> {
    &TTY_INODE
}

pub fn tty_ioctl(command: usize, value: usize) -> KResult {
    debug!("tty ioctl. command {command:#x}, value {value:#x}");
    match command {
        TCGETS | TCGETA => {
            todo!("TCGETS | TCGETA")
        }
        TCSETS | TCSETSW | TCSETSF => {
            todo!("TCSETS | TCSETSW | TCSETSF")
        }
        TIOCGPGRP => {
            debug!("Get foreground pgid");
            let fg_pgid_ptr = unsafe { UserCheck::new(value as _).check_ptr_mut()? };
            fg_pgid_ptr.write(TTY_INODE.inner.inner.lock().fg_pgid);
            Ok(0)
        }
        TIOCSPGRP => {
            debug!("Set foreground pgid");
            let fg_pgid = UserCheck::<usize>::new(value as _).check_ptr()?.read();
            TTY_INODE.inner.inner.lock().fg_pgid = fg_pgid;
            Ok(0)
        }
        TIOCGWINSZ => {
            debug!("Get window size");
            let win_size_ptr = unsafe { UserCheck::new(value as _).check_ptr_mut()? };
            win_size_ptr.write(TTY_INODE.inner.inner.lock().win_size);
            Ok(0)
        }
        TIOCSWINSZ => {
            debug!("Set window size");
            let win_size = UserCheck::<WinSize>::new(value as _).check_ptr()?.read();
            TTY_INODE.inner.inner.lock().win_size = win_size;
            Ok(0)
        }
        TCSBRK => Ok(0),
        _ => todo!("others"),
    }
}

pub struct TtyInode {
    inner: SpinMutex<TtyInodeInner>,
}

struct TtyInodeInner {
    fg_pgid: usize,
    win_size: WinSize,
}

static TTY_INODE: Lazy<Inode<TtyInode>> = Lazy::new(|| {
    Inode::new(
        InodeMeta::new(InodeMode::CharDevice, CompactString::from_static_str("/")),
        TtyInode {
            inner: SpinMutex::new(TtyInodeInner {
                fg_pgid: INITPROC.pid(),
                win_size: WinSize {
                    ws_row: 67,
                    ws_col: 120,
                    xpixel: 0,
                    ypixel: 0,
                },
            }),
        },
    )
});

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TtyFuture {
    user_buf: UserCheck<[u8]>,
}

impl TtyFuture {
    pub fn new(user_buf: UserCheck<[u8]>) -> Self {
        Self { user_buf }
    }
}

impl Future for TtyFuture {
    type Output = KResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut tty = TTY.lock();
        let mut cnt = 0;
        let user_buf = unsafe { self.user_buf.check_slice_mut()? };
        let mut out = user_buf.out();
        loop {
            let out = out.reborrow();
            if cnt >= out.len() {
                break;
            }
            if let Some(byte) = tty.get_byte() {
                // # SAFETY: 上面比较过长度因此不会越界
                unsafe { out.get_unchecked_out(cnt).write(byte) };
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
