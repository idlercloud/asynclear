use core::ops::Deref;

use alloc::collections::BTreeMap;
use bitflags::bitflags;
use defines::error::{errno, KResult};
use klocks::SpinMutex;
use triomphe::Arc;
use user_check::{UserCheck, UserCheckMut};

use super::{
    stdio::{read_stdin, write_stdout},
    DEntryDir, DEntryPaged,
};

#[derive(Clone)]
pub enum File {
    Stdin,
    Stdout,
    Dir(Arc<DEntryDir>),
    Paged(Arc<PagedFile>),
}

pub struct PagedFile {
    dentry: DEntryPaged,
    offset: SpinMutex<usize>,
}

impl PagedFile {
    pub fn new(dentry: DEntryPaged) -> Self {
        Self {
            dentry,
            offset: SpinMutex::new(0),
        }
    }
}

#[derive(Clone)]
pub struct FdTable {
    files: BTreeMap<usize, FileDescriptor>,
}

impl FdTable {
    pub fn with_stdio() -> Self {
        let files = BTreeMap::from([
            (0, FileDescriptor::new(File::Stdin, OpenFlags::RDONLY)),
            (1, FileDescriptor::new(File::Stdout, OpenFlags::WRONLY)),
            (2, FileDescriptor::new(File::Stdout, OpenFlags::WRONLY)),
        ]);
        Self { files }
    }

    /// 添加一个描述符，并返回其 fd
    pub fn add(&mut self, descriptor: FileDescriptor) -> usize {
        let mut new_fd = 0;
        for &existed_fd in self.files.keys() {
            if new_fd != existed_fd {
                break;
            }
            new_fd += 1;
        }
        self.files.insert(new_fd, descriptor);
        new_fd
    }

    pub fn get(&self, fd: usize) -> Option<&FileDescriptor> {
        self.files.get(&fd)
    }

    pub fn remove(&mut self, fd: usize) -> Option<FileDescriptor> {
        self.files.remove(&fd)
    }

    pub fn close_on_exec(&mut self) {
        self.files
            .retain(|fd, file| !file.flags.contains(OpenFlags::CLOEXEC));
    }
}

#[derive(Clone)]
pub struct FileDescriptor {
    file: File,
    flags: OpenFlags,
}

impl FileDescriptor {
    pub fn new(file: File, flags: OpenFlags) -> Self {
        Self { file, flags }
    }

    pub fn readable(&self) -> bool {
        self.flags.read_write().0
    }

    pub fn writable(&self) -> bool {
        self.flags.read_write().1
    }

    pub async fn read(&self, buf: UserCheckMut<[u8]>) -> KResult<usize> {
        match &self.file {
            File::Stdin => read_stdin(buf).await,
            File::Stdout | File::Dir(_) => Err(errno::EBADF),
            File::Paged(paged) => {
                let inode = paged.dentry.inode();
                let meta = inode.meta();
                let mut offset = paged.offset.lock();
                let nread = inode
                    .inner
                    .read_at(meta, &mut buf.check_slice_mut()?, *offset)?;
                *offset += nread;
                Ok(nread)
            }
        }
    }

    pub async fn write(&self, buf: UserCheck<[u8]>) -> KResult<usize> {
        match &self.file {
            File::Stdin | File::Dir(_) => Err(errno::EBADF),
            File::Stdout => write_stdout(buf),
            File::Paged(paged) => {
                let inode = paged.dentry.inode();
                let meta = inode.meta();
                let mut offset = paged.offset.lock();
                if self.flags.contains(OpenFlags::APPEND) {
                    *offset = inode.inner.data_len();
                }
                let nwrite = inode
                    .inner
                    .write_at(meta, &mut buf.check_slice()?, *offset)?;
                *offset += nwrite;
                Ok(nwrite)
            }
        }
    }
}

impl Deref for FileDescriptor {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

bitflags! {
    /// 注意低 2 位指出文件的打开模式
    /// 0、1、2 分别对应只读、只写、可读可写。3 为错误。
    #[derive(Clone, Copy, Debug)]
    pub struct OpenFlags: u32 {
        const RDONLY    = 0;
        const WRONLY    = 1 << 0;
        const RDWR      = 1 << 1;

        /// 如果所查询的路径不存在，则在该路径创建一个常规文件
        const CREATE    = 1 << 6;
        /// 在创建文件的情况下，保证该文件之前不存在，否则返回错误
        const EXCL      = 1 << 7;
        /// 如果路径指向一个终端设备，那么它不会成为本进程的控制终端
        const NOCTTY    = 1 << 8;
        /// 如果是常规文件，且允许写入，则将该文件长度截断为 0
        const TRUNCATE  = 1 << 9;
        /// 写入追加到文件末尾，它是在每次 `sys_write` 时生效
        const APPEND    = 1 << 10;
        /// 保持文件数据与磁盘阻塞同步。但如果该写操作不影响后续的读取，则不会同步更新元数据
        const DSYNC     = 1 << 12;
        /// 文件操作完成时发出信号
        const ASYNC     = 1 << 13;
        /// 不经过缓存，直接写入磁盘中
        const DIRECT    = 1 << 14;
        /// 允许打开文件大小超过 32 位表示范围的大文件。在 64 位系统上此标志位应永远为真
        const LARGEFILE = 1 << 15;
        /// 如果打开的文件不是目录，那么就返回失败
        const DIRECTORY = 1 << 16;
        // /// 如果路径的 basename 是一个符号链接，则打开失败并返回 `ELOOP`，目前不支持
        // const O_NOFOLLOW    = 1 << 17;
        // /// 读文件时不更新文件的 last access time，暂不支持
        // const O_NOATIME     = 1 << 18;
        /// 设置打开的文件描述符的 close-on-exec 标志
        const CLOEXEC   = 1 << 19;
        // /// 仅打开一个文件描述符，而不实际打开文件。后续只允许进行纯文件描述符级别的操作
        // TODO: 可能要考虑加上 O_PATH，似乎在某些情况下无法打开的文件可以通过它打开
        // const O_PATH        = 1 << 21;
    }
}

impl OpenFlags {
    pub fn read_write(&self) -> (bool, bool) {
        match self.bits() & 0b11 {
            0 => (true, false),
            1 => (false, true),
            2 => (true, true),
            _ => unreachable!(),
        }
    }
}
