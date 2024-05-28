use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use defines::{
    error::{errno, KResult},
    ioctl::{
        Termios, WinSize, TCGETA, TCGETS, TCSBRK, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGWINSZ,
        TIOCSPGRP, TIOCSWINSZ,
    },
};
use klocks::{Lazy, SpinMutex};
use triomphe::Arc;

use super::{
    inode::{InodeMeta, InodeMode},
    File,
};
use crate::{drivers::qemu_uart::TTY, memory::UserCheck, time, uart_console::print};

pub struct TtyInode {
    meta: InodeMeta,
    inner: SpinMutex<TtyInodeInner>,
}

struct TtyInodeInner {
    fg_pgid: usize,
    win_size: WinSize,
    termios: Termios,
}

impl TtyInode {
    pub fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    pub async fn read(&self, buf: UserCheck<[u8]>) -> KResult<usize> {
        let curr_time = time::curr_time_spec();
        self.meta
            .lock_inner_with(|inner| inner.access_time = curr_time);
        TtyFuture::new(buf).await
    }

    pub fn write(&self, buf: UserCheck<[u8]>) -> KResult<usize> {
        let curr_time = time::curr_time_spec();
        self.meta
            .lock_inner_with(|inner| inner.modify_time = curr_time);
        let buf = buf.check_slice()?;
        let s = core::str::from_utf8(&buf).unwrap();
        print!("{s}");
        Ok(buf.len())
    }

    pub fn ioctl(&self, cmd: usize, value: usize) -> KResult {
        let _enter =
            debug_span!("tty_ioctl", cmd = compact_str::format_compact!("{cmd:x}")).entered();
        match cmd {
            TCGETS | TCGETA => {
                debug!("Get termios");
                let termios_ptr = unsafe {
                    UserCheck::new(value as _)
                        .ok_or(errno::EINVAL)?
                        .check_ptr_mut()?
                };
                termios_ptr.write(self.inner.lock().termios);
                Ok(0)
            }
            TCSETS | TCSETSW | TCSETSF => {
                debug!("Set termios");
                let termios = UserCheck::<Termios>::new(value as _)
                    .ok_or(errno::EINVAL)?
                    .check_ptr()?
                    .read();
                self.inner.lock().termios = termios;
                Ok(0)
            }
            TIOCGPGRP => {
                debug!("Get foreground pgid");
                let fg_pgid_ptr = unsafe {
                    UserCheck::new(value as _)
                        .ok_or(errno::EINVAL)?
                        .check_ptr_mut()?
                };
                fg_pgid_ptr.write(self.inner.lock().fg_pgid);
                Ok(0)
            }
            TIOCSPGRP => {
                debug!("Set foreground pgid");
                let fg_pgid = UserCheck::<usize>::new(value as _)
                    .ok_or(errno::EINVAL)?
                    .check_ptr()?
                    .read();
                self.inner.lock().fg_pgid = fg_pgid;
                Ok(0)
            }
            TIOCGWINSZ => {
                debug!("Get window size");
                let win_size_ptr = unsafe {
                    UserCheck::new(value as _)
                        .ok_or(errno::EINVAL)?
                        .check_ptr_mut()?
                };
                win_size_ptr.write(self.inner.lock().win_size);
                Ok(0)
            }
            TIOCSWINSZ => {
                debug!("Set window size");
                let win_size = UserCheck::<WinSize>::new(value as _)
                    .ok_or(errno::EINVAL)?
                    .check_ptr()?
                    .read();
                self.inner.lock().win_size = win_size;
                Ok(0)
            }
            TCSBRK => {
                debug!("Send break");
                Ok(0)
            }
            _ => todo!("[low] other tty ioctl command"),
        }
    }
}

static TTY_INODE: Lazy<Arc<TtyInode>> = Lazy::new(|| {
    Arc::new({
        TtyInode {
            meta: InodeMeta::new(InodeMode::CharDevice),
            inner: SpinMutex::new(TtyInodeInner {
                fg_pgid: 1,
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
        }
    })
});

pub fn default_tty_file() -> File {
    File::Tty(Arc::clone(&TTY_INODE))
}

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
        let mut buf = unsafe { self.user_buf.check_slice_mut()? };
        let buf = buf.as_bytes_mut();
        loop {
            if cnt >= buf.len() {
                break;
            }
            if let Some(byte) = tty.get_byte() {
                buf[cnt] = byte;
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
