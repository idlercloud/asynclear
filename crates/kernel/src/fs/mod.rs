mod fat32;
mod inode;
mod page_cache;
mod stdio;

use core::ops::Deref;

use alloc::{boxed::Box, collections::BTreeMap};
use triomphe::Arc;

use async_trait::async_trait;
use bitflags::bitflags;
use defines::error::Result;
use user_check::{UserCheck, UserCheckMut};

use self::{
    // fat32::FAT_FS,
    stdio::{read_stdin, write_stdout},
};

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

    pub fn get(&self, fd: usize) -> Option<&FileDescriptor> {
        self.files.get(&fd)
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
        const CREAT     = 1 << 6;
        /// 在创建文件的情况下，保证该文件之前已经已存在，否则返回错误
        const EXCL      = 1 << 7;
        /// 如果路径指向一个终端设备，那么它不会称为本进程的控制终端
        const NOCTTY    = 1 << 8;
        /// 如果是常规文件，且允许写入，则将该文件长度截断为 0
        const TRUNC     = 1 << 9;
        /// 写入追加到文件末尾，可能在每次 `sys_write` 都有影响，暂时不支持
        const APPEND    = 1 << 10;
        /// 保持文件数据与磁盘阻塞同步。但如果该写操作不影响读取刚写入的数据，则不会等到元数据更新，暂不支持
        const DSYNC     = 1 << 12;
        /// 文件操作完成时发出信号，暂时不支持
        const ASYNC     = 1 << 13;
        /// 不经过缓存，直接写入磁盘中。目前实现仍然经过缓存
        const DIRECT    = 1 << 14;
        /// 允许打开文件大小超过 32 位表示范围的大文件。在 64 位系统上此标志位应永远为真
        const LARGEFILE = 1 << 15;
        /// 如果打开的文件不是目录，那么就返回失败
        ///
        /// FIXME: 在测试中，似乎 1 << 21 才被认为是 O_DIRECTORY；但 musl 似乎认为是 1 << 16
        const DIRECTORY = 1 << 21;
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

#[derive(Clone)]
pub enum File {
    Stdin,
    Stdout,
    #[allow(unused)]
    DynFile(Arc<dyn DynFile>),
}

impl File {
    pub async fn read(&self, buf: UserCheckMut<[u8]>) -> Result<usize> {
        match self {
            File::Stdin => read_stdin(buf).await,
            File::Stdout => panic!("stdout cannot be read"),
            File::DynFile(dyn_file) => dyn_file.read(buf).await,
        }
    }

    pub async fn write(&self, buf: UserCheck<[u8]>) -> Result<usize> {
        match self {
            File::Stdin => panic!("stdin cannot be written"),
            File::Stdout => write_stdout(buf),
            File::DynFile(dyn_file) => dyn_file.write(buf).await,
        }
    }

    pub fn is_dir(&self) -> bool {
        false
    }
}

#[async_trait]
pub trait DynFile: Send + Sync {
    async fn read(&self, buf: UserCheckMut<[u8]>) -> Result<usize>;
    async fn write(&self, buf: UserCheck<[u8]>) -> Result<usize>;
}

// pub fn init() {
//     Lazy::force(&FAT_FS);
// }
