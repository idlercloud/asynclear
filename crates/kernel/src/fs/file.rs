use alloc::collections::BTreeMap;
use core::ops::Deref;

use bitflags::bitflags;
use defines::{
    error::{errno, KResult},
    fs::{Dirent64, NAME_MAX},
};
use klocks::SpinMutex;
use triomphe::Arc;
use user_check::{UserCheck, UserCheckMut};

use super::{
    inode::{DynDirInode, DynPagedInode, InodeMeta},
    stdio, DEntry, DEntryDir, DEntryPaged,
};

#[derive(Clone)]
pub enum File {
    Stdin,
    Stdout,
    Dir(Arc<DirFile>),
    Paged(Arc<PagedFile>),
}

pub struct DirFile {
    dentry: Arc<DEntryDir>,
    dirent_index: SpinMutex<usize>,
}

impl DirFile {
    pub fn new(dentry: Arc<DEntryDir>) -> Self {
        Self {
            dentry,
            dirent_index: SpinMutex::new(0),
        }
    }

    pub fn dentry(&self) -> &Arc<DEntryDir> {
        &self.dentry
    }

    pub fn getdirents(&self, buf: &mut [u8]) -> KResult<usize> {
        self.dentry.read_dir()?;

        let children = self.dentry.lock_children();
        let mut dirent_index = self.dirent_index.lock();

        let mut ptr = buf.as_mut_ptr();
        let range = (ptr as usize)..(ptr as usize + buf.len());

        let children_iter = children
            .iter()
            .filter_map(|(name, child)| child.as_ref().map(|child| (name, child)))
            .skip(*dirent_index);
        for (name, child) in children_iter {
            use core::mem::{align_of, offset_of};
            let name_len = name.len().min(NAME_MAX);
            let mut d_reclen = (offset_of!(Dirent64, d_name) + name_len + 1);
            if ptr as usize + d_reclen > range.end {
                break;
            }
            d_reclen = d_reclen.next_multiple_of(align_of::<Dirent64>());
            let meta = child.meta();
            // SAFETY:
            // 写入范围不会重叠，且由上面控制不会写出超过 buf 的区域
            #[allow(clippy::cast_ptr_alignment)]
            unsafe {
                // NOTE: 不知道这里要不要把对齐的部分用 0 填充
                ptr.cast::<u64>().write_unaligned(meta.ino() as u64);
                // 忽略 `d_off` 字段
                ptr.add(offset_of!(Dirent64, d_reclen))
                    .cast::<u16>()
                    .write_unaligned(d_reclen as u16);
                ptr.add(offset_of!(Dirent64, d_type))
                    .write((meta.mode().bits() >> 12) as u8);
                ptr.add(offset_of!(Dirent64, d_name))
                    .copy_from_nonoverlapping(name.as_bytes()[0..name_len].as_ptr(), name_len);
                // 名字是 null-terminated 的
                ptr.add(d_reclen - 1).write(0);
                ptr = ptr.add(d_reclen);
            }
            *dirent_index += 1;
        }

        Ok(ptr as usize - range.start)
    }
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

    pub fn inode(&self) -> &DynPagedInode {
        self.dentry.inode()
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

    /// 找到最小可用的 fd，插入一个描述符，并返回该 fd
    pub fn add(&mut self, desc: FileDescriptor) -> usize {
        self.add_from(desc, 0)
    }

    /// 找到自 `from` 最小可用的 fd，插入一个描述符，并返回该 fd
    pub fn add_from(&mut self, desc: FileDescriptor, from: usize) -> usize {
        let mut new_fd = 0;
        for (&existed_fd, _) in self.files.range(from..) {
            if new_fd != existed_fd {
                break;
            }
            new_fd += 1;
        }
        self.files.insert(new_fd, desc);
        new_fd
    }

    pub fn get(&self, fd: usize) -> Option<&FileDescriptor> {
        self.files.get(&fd)
    }

    pub fn get_mut(&mut self, fd: usize) -> Option<&mut FileDescriptor> {
        self.files.get_mut(&fd)
    }

    pub fn insert(&mut self, fd: usize, desc: FileDescriptor) -> Option<FileDescriptor> {
        self.files.insert(fd, desc)
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
            File::Stdin => stdio::read_stdin(buf).await,
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
            File::Stdout => stdio::write_stdout(buf),
            File::Paged(paged) => {
                let inode = paged.dentry.inode();
                let meta = inode.meta();
                let mut offset = paged.offset.lock();
                if self.flags.contains(OpenFlags::APPEND) {
                    *offset = inode.inner.data_len();
                }
                let nwrite = inode.inner.write_at(meta, &buf.check_slice()?, *offset)?;
                *offset += nwrite;
                Ok(nwrite)
            }
        }
    }

    pub fn meta(&self) -> &InodeMeta {
        match &self.file {
            File::Stdin | File::Stdout => stdio::get_tty_inode().meta(),
            File::Dir(dir) => dir.dentry.inode().meta(),
            File::Paged(paged) => paged.dentry.inode().meta(),
        }
    }

    pub fn ioctl(&self, request: usize, argp: usize) -> KResult {
        match &self.file {
            File::Stdin | File::Stdout => stdio::tty_ioctl(request, argp),
            _ => Err(errno::ENOTTY),
        }
    }

    pub fn set_close_on_exec(&mut self, set: bool) {
        self.flags.set(OpenFlags::CLOEXEC, set);
    }

    pub fn flags(&self) -> OpenFlags {
        self.flags
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
        // /// 如果是常规文件，且允许写入，则将该文件长度截断为 0
        // const TRUNCATE  = 1 << 9;
        /// 写入追加到文件末尾，它是在每次 `sys_write` 时生效
        const APPEND    = 1 << 10;
        /// 在可能的情况下，让该文件以非阻塞模式打开
        const NONBLOCK  = 1 << 11;
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
