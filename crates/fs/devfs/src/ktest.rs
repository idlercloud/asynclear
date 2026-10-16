//! `/dev/ktest`：内核测试的结果上报通道，写入一个字节即以该值作为整轮测试的结果码关闭内核
//!
//! 只在 ktest 模式下注册这个节点，因此正常启动时它根本不存在

use alloc::boxed::Box;

use common::config::PAGE_SIZE;
use defines::error::{errno, AKResult};
use libkernel::{
    fs::inode::{BytesInodeBackend, InodeMeta, InodeMode},
    memory::{ReadBuffer, WriteBuffer},
};

pub struct KtestInode {
    meta: InodeMeta,
}

impl KtestInode {
    pub fn new() -> Self {
        let mut meta = InodeMeta::new(InodeMode::CharDevice);
        // 设备节点不关心长度，这里只是给页缓存一个上限
        meta.get_inner_mut().data_len = PAGE_SIZE as u64;
        Self { meta }
    }
}

impl BytesInodeBackend for KtestInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at<'a>(&'a self, _buf: ReadBuffer<'a>, _offset: u64) -> AKResult<'a, usize> {
        Box::pin(async move { Err(errno::EINVAL) })
    }

    fn write_inode_at<'a>(&'a self, buf: WriteBuffer<'a>, _offset: u64) -> AKResult<'a, usize> {
        Box::pin(async move {
            let result = match &buf {
                WriteBuffer::Kernel(buf) => buf.first().copied(),
                WriteBuffer::User(buf) => buf.check_slice()?.first().copied(),
            }
            .ok_or(errno::EINVAL)?;

            // 写入 sifive_test 后整个模拟结束，不会返回
            qemu_test::exit_with(result)
        })
    }
}
