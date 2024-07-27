use alloc::boxed::Box;

use common::config::PAGE_SIZE;
use defines::error::{errno, AKResult, KResult};

use crate::{
    fs::{
        inode::{BytesInodeBackend, InodeMeta},
        InodeMode, VFS,
    },
    memory::{ReadBuffer, WriteBuffer},
    time,
};

pub struct RtcInode {
    meta: InodeMeta,
}

impl RtcInode {
    pub fn new() -> Self {
        let mut meta = InodeMeta::new(InodeMode::CharDevice);
        let meta_inner = meta.get_inner_mut();
        // TODO: [low] hack: `rtc` 的大小设置为 `PAGE_SIZE`
        meta_inner.data_len = PAGE_SIZE as u64;
        let curr_time = time::curr_time_spec();
        meta_inner.access_time = curr_time;
        meta_inner.change_time = curr_time;
        meta_inner.modify_time = curr_time;
        Self { meta }
    }
}

impl BytesInodeBackend for RtcInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at<'a>(&'a self, buf: ReadBuffer<'a>, offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move {
            debug!("read rtc");
            // TODO: [low] rtc 实现不正确
            let n_read = buf.len();
            match buf {
                ReadBuffer::Kernel(buf) => buf.fill(0),
                ReadBuffer::User(buf) => unsafe {
                    buf.check_slice_mut()?.as_bytes_mut().fill(0);
                },
            }

            Ok(n_read)
        })
    }

    fn write_inode_at<'a>(&'a self, _buf: WriteBuffer<'a>, _offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move { Err(errno::EBADF) })
    }

    fn ioctl(&self, request: usize, argp: usize) -> KResult {
        // TODO: [low] rtc 的 ioctl 不知道该怎么实现
        Ok(0)
    }
}
