use alloc::boxed::Box;

use common::config::PAGE_SIZE;
use defines::error::{errno, AKResult};

use crate::{
    fs::{
        inode::{BytesInodeBackend, InodeMeta},
        InodeMode, VFS,
    },
    memory::{ReadBuffer, WriteBuffer},
    time,
};

pub struct MeminfoInode {
    meta: InodeMeta,
}

impl MeminfoInode {
    pub fn new() -> Self {
        let mut meta = InodeMeta::new(InodeMode::Regular);
        let meta_inner = meta.get_inner_mut();
        // TODO: [low] hack: `meminfo` 的大小设置为 `PAGE_SIZE`
        meta_inner.data_len = PAGE_SIZE as u64;
        let curr_time = time::curr_time_spec();
        meta_inner.access_time = curr_time;
        meta_inner.change_time = curr_time;
        meta_inner.modify_time = curr_time;
        Self { meta }
    }
}

static DUMMY_MEMINFO: &[u8] = b"\
MemTotal:\t1919810KB
MemFree:\t114514KB
MemAvailable:\t142857KB
Buffers:\t10000KB
Cached:\t20000KB
SwapCached:\t30000KB
SwapTotal:\t40000KB
SwapFree:\t50000KB
Shmem:\t60000KB
Slab:\t70000KB
";

impl BytesInodeBackend for MeminfoInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at<'a>(&'a self, buf: ReadBuffer<'a>, offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move {
            // TODO: [low] 目前对这类伪文件系统中文件的读取实现并不正确，要求传入的 buf 能够一次性读取所有内容
            debug!("read meminfo");
            assert_eq!(offset, 0);
            let read_len = usize::min(buf.len(), DUMMY_MEMINFO.len());
            assert_eq!(read_len, DUMMY_MEMINFO.len());
            match buf {
                ReadBuffer::Kernel(buf) => {
                    buf[0..read_len].copy_from_slice(&DUMMY_MEMINFO[..read_len]);
                }
                ReadBuffer::User(buf) => unsafe {
                    buf.slice(0..read_len)
                        .expect("must be in bound")
                        .check_slice_mut()?
                        .as_bytes_mut()
                        .copy_from_slice(&DUMMY_MEMINFO[..read_len]);
                },
            }

            Ok(read_len)
        })
    }

    fn write_inode_at<'a>(&'a self, _buf: WriteBuffer<'a>, _offset: u64) -> AKResult<'_, usize> {
        Box::pin(async move { Err(errno::EBADF) })
    }
}
