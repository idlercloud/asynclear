use alloc::sync::{Arc, Weak};

use async_channel::{Receiver, Sender};
use async_lock::Mutex as SleepMutex;
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::TimeSpec,
};
use heapless::mpmc::MpMcQueue;

use super::{inode::InodeMeta, InodeMode};
use crate::{memory::UserCheck, time};

// TODO: [low] pipe 的实现可以优化

const PIPE_CAPACITY: usize = 16384;

#[derive(Clone)]
pub struct Pipe {
    meta: Arc<InodeMeta>,
    inner: PipeInner,
}

impl Pipe {
    pub async fn read(&self, buf: UserCheck<[u8]>) -> KResult<usize> {
        let PipeInner::ReadEnd(receiver) = &self.inner else {
            return Err(errno::EBADF);
        };
        let buf = unsafe { buf.check_slice_mut()? };
        let mut out = buf.out();
        let mut n_read = 0;

        while let Some(ptr) = out.reborrow().get_out(n_read)
            && let Ok(byte) = receiver.recv().await
        {
            ptr.write(byte);
            n_read += 1;
        }
        self.meta.lock_inner_with(|inner| {
            inner.access_time = TimeSpec::from(time::curr_time());
        });
        Ok(n_read)
    }

    pub async fn write(&self, buf: UserCheck<[u8]>) -> KResult<usize> {
        let PipeInner::WriteEnd(sender) = &self.inner else {
            return Err(errno::EBADF);
        };
        let buf = buf.check_slice()?;
        let mut n_write = 0;

        for &byte in &*buf {
            if sender.send(byte).await.is_err() {
                break;
            }
            n_write += 1;
        }

        self.meta.lock_inner_with(|inner| {
            inner.modify_time = TimeSpec::from(time::curr_time());
        });
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

#[derive(Clone)]
pub struct WritePipe {
    sender: Sender<u8>,
}

/// 返回 (`read_end`, `write_end`)
pub fn make_pipe() -> (Pipe, Pipe) {
    let (sender, receiver) = async_channel::bounded(PIPE_CAPACITY);
    let meta = Arc::new(InodeMeta::new(
        InodeMode::Fifo,
        CompactString::from_static_str("_pipe"),
    ));
    meta.lock_inner_with(|inner| {
        inner.data_len = PIPE_CAPACITY as u64;
        inner.change_time = TimeSpec::from(time::curr_time());
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
