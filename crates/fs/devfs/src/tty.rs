use alloc::boxed::Box;
use core::{future, task::Poll};

use console_output::print;
use defines::{
    error::{AKResult, KResult},
    ioctl::{
        Termios, WinSize, TCGETA, TCGETS, TCSBRK, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGWINSZ, TIOCSPGRP,
        TIOCSWINSZ,
    },
};
use executor::time;
use kernel_tracer::Instrument;
use klocks::SpinMutex;
use libkernel::{
    fs::inode::{BytesInodeBackend, InodeMeta, InodeMode},
    memory::{ReadBuffer, UserPtr, WriteBuffer},
};
use qemu_uart::UART0;
use scopeguard::defer;

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
        Box::pin(async move {
            let buf = UserPtr::new_slice(buf.ptr.as_ptr().cast::<u8>(), buf.len())?;
            read_tty(buf).instrument(trace_span!("read_tty")).await
        })
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
                let termios_ptr = UserPtr::new(value as *mut Termios)?;
                // let termios_ptr = unsafe { UserCheckLegacy::new(value as _).ok_or(errno::EINVAL)?.check_ptr_mut()? };
                unsafe {
                    termios_ptr.write(self.inner.lock().termios)?;
                }
                Ok(0)
            }
            TCSETS | TCSETSW | TCSETSF => {
                debug!("Set termios");
                let termios_ptr = UserPtr::new(value as *mut Termios)?;
                self.inner.lock().termios = unsafe { termios_ptr.read()? };
                Ok(0)
            }
            TIOCGPGRP => {
                debug!("Get foreground pgid");
                let fg_pgid_ptr = UserPtr::new(value as *mut usize)?;
                unsafe {
                    fg_pgid_ptr.write(self.inner.lock().fg_pgid)?;
                }
                Ok(0)
            }
            TIOCSPGRP => {
                debug!("Set foreground pgid");
                let fg_pgid_ptr = UserPtr::new(value as *mut usize)?;
                self.inner.lock().fg_pgid = unsafe { fg_pgid_ptr.read()? };
                Ok(0)
            }
            TIOCGWINSZ => {
                debug!("Get window size");
                let win_size_ptr = UserPtr::new(value as *mut WinSize)?;
                unsafe {
                    win_size_ptr.write(self.inner.lock().win_size)?;
                }
                Ok(0)
            }
            TIOCSWINSZ => {
                debug!("Set window size");
                let win_size_ptr = UserPtr::new(value as *mut WinSize)?;
                self.inner.lock().win_size = unsafe { win_size_ptr.read()? };
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

async fn read_tty(user_buf: UserPtr<[u8]>) -> KResult<usize> {
    let user_buf_len = user_buf.len();
    if user_buf_len == 0 {
        return Ok(0);
    }
    let mut consumer = UART0.lock_consumer().await;
    // 保证只有持有 consumer 锁的任务才会注册 waker
    defer! {
        UART0.waker.take();
    }
    future::poll_fn(move |cx| {
        if consumer.is_empty() {
            UART0.waker.register(cx.waker());
        }
        // 先注册，避免 race condition
        // 具体而言，如果先检查到无数据，再注册 waker
        // 那么在这两个操作中间可能有个微小的窗口期，如果这时来了数据，那么调用 `wake()` 是无操作
        // 这个任务就可能错过唤醒了
        let mut cnt = 0;
        while cnt < user_buf_len {
            // 临时栈上缓冲区，先从 consumer 中 dequeue 数据放到这里
            let mut stack_buf = heapless::Vec::<u8, 32>::new();
            while cnt + stack_buf.len() < user_buf_len && !stack_buf.is_full() {
                if let Some(byte) = consumer.dequeue() {
                    stack_buf.push(byte).unwrap();
                } else {
                    break;
                }
            }
            if stack_buf.is_empty() {
                break;
            }

            let batch_len = stack_buf.len();
            if let Err(err) = unsafe { user_buf.write_bytes_at(&stack_buf, cnt) } {
                return Poll::Ready(Err(err));
            }
            cnt += batch_len;
        }
        if cnt > 0 {
            UART0.waker.take();
            Poll::Ready(Ok(cnt))
        } else {
            Poll::Pending
        }
    })
    .await
}
