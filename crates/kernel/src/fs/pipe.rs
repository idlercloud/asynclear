use async_channel::{Receiver, Sender};
use defines::error::{errno, KResult};
use triomphe::Arc;

use super::{inode::InodeMeta, InodeMode};
use crate::{
    memory::{ReadBuffer, UserCheck, WriteBuffer},
    time,
};

// TODO: [low] pipe 的实现可以优化

const PIPE_CAPACITY: usize = 16384;

#[derive(Clone)]
pub struct Pipe {
    meta: Arc<InodeMeta>,
    inner: PipeInner,
}

impl Pipe {
    pub async fn read(&self, mut buf: ReadBuffer<'_>) -> KResult<usize> {
        let PipeInner::ReadEnd(receiver) = &self.inner else {
            return Err(errno::EBADF);
        };
        let len = buf.len();
        let mut n_read = 0;

        while n_read < len {
            let Ok(byte) = receiver.recv().await else {
                break;
            };
            match &mut buf {
                ReadBuffer::Kernel(buf) => buf[n_read] = byte,
                ReadBuffer::User(buf) => unsafe {
                    buf.as_user_check()
                        .add(n_read)
                        .ok_or(errno::EINVAL)?
                        .check_ptr_mut()?
                        .write(byte);
                },
            }
            n_read += 1;
        }
        let curr_time = time::curr_time_spec();
        self.meta.lock_inner_with(|inner| inner.access_time = curr_time);
        Ok(n_read)
    }

    pub async fn write(&self, mut buf: WriteBuffer<'_>) -> KResult<usize> {
        let PipeInner::WriteEnd(sender) = &self.inner else {
            return Err(errno::EBADF);
        };
        let len = buf.len();
        let mut n_write = 0;

        'out: loop {
            let mut this_n_write = 0;
            let last_byte = {
                let slice = match &buf {
                    WriteBuffer::Kernel(buf) => *buf,
                    WriteBuffer::User(buf) => &buf.check_slice()?,
                };
                for &byte in slice.iter() {
                    if let Err(err) = sender.try_send(byte) {
                        this_n_write += 1;
                        n_write += this_n_write;
                        break 'out;
                    }
                    this_n_write += 1;
                }
                if this_n_write < slice.len() {
                    slice[this_n_write]
                } else {
                    n_write += this_n_write;
                    break 'out;
                }
            };
            sender.send(last_byte).await;
            this_n_write += 1;
            n_write += this_n_write;
            buf = buf.slice(this_n_write..buf.len()).expect("this_n_write <= buf.len()");
        }

        let curr_time = time::curr_time_spec();
        self.meta.lock_inner_with(|inner| inner.modify_time = curr_time);
        Ok(n_write)
    }

    pub fn meta(&self) -> &InodeMeta {
        &self.meta
    }
}

#[derive(Clone)]
enum PipeInner {
    ReadEnd(Receiver<u8>),
    WriteEnd(Sender<u8>),
}

/// 返回 (`read_end`, `write_end`)
pub fn make_pipe() -> (Pipe, Pipe) {
    let (sender, receiver) = async_channel::bounded(PIPE_CAPACITY);
    let meta = Arc::new(InodeMeta::new(InodeMode::Fifo));
    let curr_time = time::curr_time_spec();
    meta.lock_inner_with(|inner| {
        inner.data_len = PIPE_CAPACITY as u64;
        inner.change_time = curr_time;
    });
    (
        Pipe {
            meta: Arc::clone(&meta),
            inner: PipeInner::ReadEnd(receiver),
        },
        Pipe {
            meta,
            inner: PipeInner::WriteEnd(sender),
        },
    )
}

//     fn fstat(&self) -> Stat {
//         Stat {
//             st_mode: StatMode::S_IFIFO | StatMode::S_IRWXU | StatMode::S_IRWXG | StatMode::S_IRWXO,
//             st_size: RING_BUFFER_SIZE as u64,
//             st_blksize: BLOCK_SIZE,
//             ..Default::default()
//         }
//     }

//     fn remove(&self, _name: &str) {
//         panic!("pipe cannot remove");
//     }
// }
