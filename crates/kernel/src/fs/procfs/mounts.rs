use alloc::boxed::Box;

use common::config::PAGE_SIZE;
use compact_str::CompactString;
use defines::error::{errno, AKResult};

use crate::{
    fs::{
        inode::{BytesInodeBackend, InodeMeta},
        InodeMode, VFS,
    },
    memory::{ReadBuffer, WriteBuffer},
    time,
};

pub struct MountsInode {
    meta: InodeMeta,
}

impl MountsInode {
    pub fn new() -> Self {
        let mut meta = InodeMeta::new(InodeMode::Regular);
        let meta_inner = meta.get_inner_mut();
        // TODO: [low] hack: `mounts` 的大小设置为 `PAGE_SIZE`
        meta_inner.data_len = PAGE_SIZE as u64;
        let curr_time = time::curr_time_spec();
        meta_inner.access_time = curr_time;
        meta_inner.change_time = curr_time;
        meta_inner.modify_time = curr_time;
        Self { meta }
    }
}

impl BytesInodeBackend for MountsInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at<'a>(&'a self, buf: ReadBuffer<'a>, offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move {
            // TODO: [low] 目前对这类伪文件系统中文件的读取实现并不正确，要求传入的 buf 能够一次性读取所有内容
            debug!("read mounts info");
            assert_eq!(offset, 0);
            let mounts_info = VFS.mounts_info();
            let mounts_info = mounts_info.as_bytes();
            let read_len = usize::min(buf.len(), mounts_info.len());
            assert_eq!(read_len, mounts_info.len());
            match buf {
                ReadBuffer::Kernel(buf) => {
                    buf[0..read_len].copy_from_slice(&mounts_info[..read_len])
                }
                ReadBuffer::User(buf) => unsafe {
                    buf.slice(0..read_len)
                        .expect("must be in bound")
                        .check_slice_mut()?
                        .as_bytes_mut()
                        .copy_from_slice(&mounts_info[..read_len])
                },
            }
            VFS.mount_table.lock();

            Ok(read_len)
        })
    }

    fn write_inode_at<'a>(&'a self, _buf: WriteBuffer<'a>, _offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move { Err(errno::EBADF) })
    }
}
