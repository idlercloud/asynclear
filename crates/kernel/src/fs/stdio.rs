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

pub fn tty_ioctl(cmd: usize, value: usize) -> KResult {
    let _enter = debug_span!("tty_ioctl", cmd = cmd, value = value).entered();
    match cmd {
        TCGETS | TCGETA => {
            debug!("Get termios");
            let termios_ptr = unsafe { UserCheck::new(value as _).check_ptr_mut()? };
            termios_ptr.write(TTY_INODE.inner.lock().termios);
            Ok(0)
        }
        TCSETS | TCSETSW | TCSETSF => {
            debug!("Set termios");
            let termios = UserCheck::<Termios>::new(value as _).check_ptr()?.read();
            TTY_INODE.inner.lock().termios = termios;
            Ok(0)
        }
        TIOCGPGRP => {
            debug!("Get foreground pgid");
            let fg_pgid_ptr = unsafe { UserCheck::new(value as _).check_ptr_mut()? };
            fg_pgid_ptr.write(TTY_INODE.inner.lock().fg_pgid);
            Ok(0)
        }
        TIOCSPGRP => {
            debug!("Set foreground pgid");
            let fg_pgid = UserCheck::<usize>::new(value as _).check_ptr()?.read();
            TTY_INODE.inner.lock().fg_pgid = fg_pgid;
            Ok(0)
        }
        TIOCGWINSZ => {
            debug!("Get window size");
            let win_size_ptr = unsafe { UserCheck::new(value as _).check_ptr_mut()? };
            win_size_ptr.write(TTY_INODE.inner.lock().win_size);
            Ok(0)
        }
        TIOCSWINSZ => {
            debug!("Set window size");
            let win_size = UserCheck::<WinSize>::new(value as _).check_ptr()?.read();
            TTY_INODE.inner.lock().win_size = win_size;
            Ok(0)
        }
        TCSBRK => Ok(0),
        _ => todo!("[low] other tty ioctl command"),
    }
}

pub struct TtyInode {
    inner: SpinMutex<TtyInodeInner>,
}

struct TtyInodeInner {
    fg_pgid: usize,
    win_size: WinSize,
    termios: Termios,
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
