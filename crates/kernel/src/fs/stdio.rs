use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use compact_str::CompactString;
use defines::{
    error::KResult,
    ioctl::{
        Termios, WinSize, TCGETA, TCGETS, TCSBRK, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGWINSZ,
        TIOCSPGRP, TIOCSWINSZ,
    },
};
use klocks::{Lazy, SpinMutex};
use user_check::{UserCheck, UserCheckMut};

use super::inode::{Inode, InodeMeta, StatMode};
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
            todo!("TIOCGPGRP")
        }
        TIOCSPGRP => {
            todo!("TIOCSPGRP")
        }
        TIOCGWINSZ => {
            let mut win_size_ptr = UserCheckMut::new(value as _).check_ptr_mut()?;
            *win_size_ptr = TTY_INODE.inner.inner.lock().win_size;
            Ok(0)
        }
        TIOCSWINSZ => {
            let win_size_ptr = UserCheck::new(value as _).check_ptr()?;
            TTY_INODE.inner.inner.lock().win_size = *win_size_ptr;
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
    win_size: WinSize,
    termios: Termios,
}

static TTY_INODE: Lazy<Inode<TtyInode>> = Lazy::new(|| {
    Inode::new(
        InodeMeta::new(StatMode::CHAR_DEVICE, CompactString::from_static_str("/")),
        TtyInode {
            inner: SpinMutex::new(TtyInodeInner {
                win_size: WinSize {
                    ws_row: 67,
                    ws_col: 120,
                    xpixel: 0,
                    ypixel: 0,
                },
                termios: Termios {
                    // IMAXBEL | IUTF8 | IXON | IXANY | ICRNL | BRKINT
                    iflag: 0o66402,
                    // OPOST | ONLCR
                    oflag: 0o5,
                    // HUPCL | CREAD | CSIZE | EXTB
                    cflag: 0o2277,
                    // IEXTEN | ECHOTCL | ECHOKE ECHO | ECHOE | ECHOK | ISIG | ICANON
                    lflag: 0o105073,
                    line: 0,
                    cc: [
                        3,   // VINTR Ctrl-C
                        28,  // VQUIT
                        127, // VERASE
                        21,  // VKILL
                        4,   // VEOF Ctrl-D
                        0,   // VTIME
                        1,   // VMIN
                        0,   // VSWTC
                        17,  // VSTART
                        19,  // VSTOP
                        26,  // VSUSP Ctrl-Z
                        255, // VEOL
                        18,  // VREPAINT
                        15,  // VDISCARD
                        23,  // VWERASE
                        22,  // VLNEXT
                        255, // VEOL2
                        0, 0,
                    ],
                    // ispeed: 0,
                    // ospeed: 0,
                },
            }),
        },
    )
});

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
