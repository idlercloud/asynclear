use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use defines::{
    error::{errno, AKResult, KResult},
    ioctl::{
        Termios, WinSize, TCGETA, TCGETS, TCSBRK, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGWINSZ, TIOCSPGRP,
        TIOCSWINSZ,
    },
};
use futures::future::BoxFuture;
use kernel_tracer::Instrument;
use klocks::{Lazy, SpinMutex};
use triomphe::Arc;

use crate::{
    drivers::qemu_uart::TTY,
    fs::{
        inode::{BytesInodeBackend, InodeMeta},
        DynBytesInode, InodeMode,
    },
    memory::{ReadBuffer, UserCheck, WriteBuffer},
    time,
    uart_console::print,
};

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
    pub(super) fn new() -> Self {
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
    }
}

impl BytesInodeBackend for TtyInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at<'a>(&'a self, buf: ReadBuffer<'a>, _offset: u64) -> AKResult<'a, usize> {
        let curr_time = time::curr_time_spec();
        self.meta.lock_inner_with(|inner| inner.access_time = curr_time);
        let ReadBuffer::User(buf) = buf else {
            unreachable!("why kernel read tty?");
        };
        Box::pin(TtyFuture::new(buf).instrument(trace_span!("read_tty")))
    }

    fn write_inode_at<'a>(&'a self, buf: WriteBuffer<'a>, _offset: u64) -> AKResult<'a, usize> {
        let _entered = trace_span!("write_tty").entered();
        let curr_time = time::curr_time_spec();
        self.meta.lock_inner_with(|inner| inner.modify_time = curr_time);

        Box::pin(async move {
            let buf = match &buf {
                WriteBuffer::Kernel(buf) => *buf,
                WriteBuffer::User(buf) => &buf.check_slice()?,
            };
            let s = core::str::from_utf8(buf).unwrap();
            print!("{s}");
            Ok(buf.len())
        })
    }

    fn ioctl(&self, cmd: usize, value: usize) -> KResult {
        let _enter = debug_span!("tty_ioctl", cmd = ecow::eco_format!("{cmd:x}")).entered();
        match cmd {
            TCGETS | TCGETA => {
                debug!("Get termios");
                let termios_ptr = unsafe { UserCheck::new(value as _).ok_or(errno::EINVAL)?.check_ptr_mut()? };
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
                let fg_pgid_ptr = unsafe { UserCheck::new(value as _).ok_or(errno::EINVAL)?.check_ptr_mut()? };
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
                let win_size_ptr = unsafe { UserCheck::new(value as _).ok_or(errno::EINVAL)?.check_ptr_mut()? };
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
