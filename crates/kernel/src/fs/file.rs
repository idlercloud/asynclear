use alloc::{collections::BTreeMap, vec::Vec};
use core::{fmt::Debug, ops::Deref};

use async_lock::Mutex as SleepMutex;
use defines::{
    error::{errno, KResult},
    fs::{Dirent64, OpenFlags, StatMode, MAX_FD_NUM, NAME_MAX},
    resource::{RLimit, RLIM_INFINITY},
};
use klocks::SpinMutex;
use triomphe::Arc;

use super::{
    inode::{DynDirInode, InodeMeta, InodeMode},
    pipe::Pipe,
    DEntry, DEntryBytes, DEntryDir, DynBytesInode,
};
use crate::memory::{ReadBuffer, UserCheck, WriteBuffer};

#[derive(Clone)]
pub enum File {
    Pipe(Pipe),
    Dir(Arc<DirFile>),
    Seekable(Arc<SeekableFile>),
    Stream(Arc<DEntryBytes>),
}

impl File {
    pub fn meta(&self) -> &InodeMeta {
        match self {
            File::Dir(dir) => dir.inode().meta(),
            File::Seekable(seekable) => seekable.inode().meta(),
            File::Pipe(pipe) => pipe.meta(),
            File::Stream(stream) => stream.inode().meta(),
        }
    }
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

    pub fn inode(&self) -> &Arc<DynDirInode> {
        self.dentry.inode()
    }

    pub fn getdirents(&self, buf: &mut [u8]) -> KResult<usize> {
        self.dentry.read_dir()?;

        let children = self.dentry.lock_children();
        let mut dirent_index = self.dirent_index.lock();

        let mut ptr = buf.as_mut_ptr().cast::<u8>();
        let range = (ptr as usize)..(ptr as usize + buf.len());

        let children_iter = children.iter().skip(*dirent_index);
        for (name, child) in children_iter {
            use core::mem::{align_of, offset_of};
            let name_len = name.len().min(NAME_MAX);
            let mut d_reclen = (offset_of!(Dirent64, d_name) + name_len + 1);
            if ptr as usize + d_reclen > range.end {
                break;
            }
            d_reclen =
                (ptr as usize + d_reclen).next_multiple_of(align_of::<Dirent64>()) - ptr as usize;
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
                    .write((StatMode::from(meta.mode()).bits() >> 12) as u8);
                ptr.add(offset_of!(Dirent64, d_name))
                    .copy_from_nonoverlapping(name.as_bytes()[0..name_len].as_ptr(), name_len);
                // 名字是 null-terminated 的
                ptr.add(offset_of!(Dirent64, d_name) + name_len).write(0);
                ptr = ptr.add(d_reclen);
            }
            *dirent_index += 1;
        }

        Ok(ptr as usize - range.start)
    }
}

pub struct SeekableFile {
    dentry: Arc<DEntryBytes>,
    offset: SleepMutex<u64>,
}

impl SeekableFile {
    pub fn new(dentry: Arc<DEntryBytes>) -> Self {
        Self {
            dentry,
            offset: SleepMutex::new(0),
        }
    }

    pub fn inode(&self) -> &Arc<DynBytesInode> {
        self.dentry.inode()
    }
}

#[derive(Clone)]
pub struct FdTable {
    files: BTreeMap<usize, FileDescriptor>,
    rlimit: RLimit,
}

impl FdTable {
    pub fn with_stdio() -> Self {
        let inode = super::find_file("/dev/tty").unwrap();
        let DEntry::Bytes(bytes) = inode else {
            unreachable!("/dev/tty should not be dir");
        };
        let file = File::Stream(bytes);
        let files = BTreeMap::from([
            (0, FileDescriptor::new(file.clone(), OpenFlags::RDONLY)),
            (1, FileDescriptor::new(file.clone(), OpenFlags::WRONLY)),
            (2, FileDescriptor::new(file, OpenFlags::WRONLY)),
        ]);
        Self {
            files,
            rlimit: RLimit {
                rlim_curr: MAX_FD_NUM,
                rlim_max: RLIM_INFINITY,
            },
        }
    }

    /// 找到最小可用的 fd，插入一个描述符，并返回该 fd
    pub fn add(&mut self, desc: FileDescriptor) -> Option<usize> {
        self.add_from(desc, 0)
    }

    pub fn add_many<const N: usize>(&mut self, descs: [FileDescriptor; N]) -> Option<[usize; N]> {
        if self.files.len() + N > self.rlimit.rlim_curr {
            return None;
        }

        let mut new_fd = 0;
        let mut n_ok = 0;
        let mut ret = [usize::MAX; N];
        for &existed_fd in self.files.keys() {
            if new_fd != existed_fd {
                if n_ok >= N {
                    break;
                }
                ret[n_ok] = new_fd;
                n_ok += 1;
            }
            new_fd += 1;
        }
        while n_ok < N {
            ret[n_ok] = new_fd;
            n_ok += 1;
            new_fd += 1;
        }
        for (fd, desc) in ret.into_iter().zip(descs.into_iter()) {
            self.files.insert(fd, desc);
        }
        Some(ret)
    }

    /// 找到自 `from` 最小可用的 fd，插入一个描述符，并返回该 fd
    ///
    /// 如果超过进程文件描述符软上限则返回 None
    pub fn add_from(&mut self, desc: FileDescriptor, from: usize) -> Option<usize> {
        if self.files.len() >= self.rlimit.rlim_curr {
            return None;
        }
        let mut new_fd = from;
        for (&existed_fd, _) in self.files.range(from..) {
            if new_fd != existed_fd {
                break;
            }
            new_fd += 1;
        }
        self.files.insert(new_fd, desc);
        Some(new_fd)
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
            .retain(|_, file| !file.flags.contains(OpenFlags::CLOEXEC));
    }

    pub fn limit(&self) -> usize {
        self.rlimit.rlim_curr
    }
}

impl Debug for FdTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FdTable")
            .field("rlimit", &self.rlimit)
            .field_with("files", |f| {
                f.debug_map()
                    .entries(self.files.iter().map(|(fd, file)| (fd, file.debug_name())))
                    .finish()
            })
            .finish()
    }
}

#[derive(Clone)]
pub struct FileDescriptor {
    file: File,
    // FIXME: 实现有问题，要区分 file status flags 和 file descriptor flags
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

    pub async fn read(&self, mut buf: ReadBuffer<'_>) -> KResult<usize> {
        match &self.file {
            File::Dir(_) => Err(errno::EBADF),
            File::Pipe(pipe) => pipe.read(buf).await,
            File::Seekable(seekable) => {
                let inode = seekable.inode();
                let mut offset = seekable.offset.lock().await;
                let nread = inode.read_at(buf, *offset).await?;
                *offset += nread as u64;
                Ok(nread)
            }
            File::Stream(stream) => stream.inode().read_at(buf, 0).await,
        }
    }

    pub async fn read_at(&self, mut buf: ReadBuffer<'_>, offset: u64) -> KResult<usize> {
        match &self.file {
            File::Seekable(seekable) => {
                let inode = seekable.inode();
                let nread = inode.read_at(buf, offset).await?;
                Ok(nread)
            }
            File::Dir(_) => Err(errno::EBADF),
            File::Stream(_) | File::Pipe(_) => Err(errno::ESPIPE),
        }
    }

    pub async fn write(&self, buf: WriteBuffer<'_>) -> KResult<usize> {
        match &self.file {
            File::Seekable(seekable) => {
                let inode = seekable.inode();
                let mut offset = seekable.offset.lock().await;
                if self.flags.contains(OpenFlags::APPEND) {
                    *offset = inode.meta().lock_inner_with(|inner| inner.data_len);
                }
                let nwrite = inode.write_at(buf, *offset).await?;
                *offset += nwrite as u64;
                Ok(nwrite)
            }
            File::Stream(stream) => stream.inode().write_at(buf, 0).await,
            File::Pipe(pipe) => pipe.write(buf).await,
            File::Dir(_) => Err(errno::EBADF),
        }
    }

    pub async fn seek(&self, pos: SeekFrom) -> KResult<usize> {
        match &self.file {
            File::Seekable(seekable) => {
                let ret = match pos {
                    SeekFrom::Start(pos) => {
                        *seekable.offset.lock().await = pos;
                        pos as usize
                    }
                    SeekFrom::End(offset) => {
                        let new_pos = seekable
                            .dentry
                            .inode()
                            .meta()
                            .lock_inner_with(|inner| inner.data_len)
                            .checked_add_signed(offset)
                            .ok_or(errno::EOVERFLOW)?;
                        *seekable.offset.lock().await = new_pos;
                        new_pos as usize
                    }
                    SeekFrom::Current(pos) => {
                        let mut curr = seekable.offset.lock().await;
                        *curr = curr.checked_add_signed(pos).ok_or(errno::EOVERFLOW)?;
                        *curr as usize
                    }
                };
                Ok(ret)
            }
            File::Stream(_) | File::Pipe(_) => Err(errno::ESPIPE),
            File::Dir(_) => todo!("[low] what does dir seek mean?"),
        }
    }

    pub fn meta(&self) -> &InodeMeta {
        self.file.meta()
    }

    pub fn ioctl(&self, request: usize, argp: usize) -> KResult {
        // TODO: [low] 目前只支持字符设备，块设备不知道会不会用到
        if !matches!(self.meta().mode(), InodeMode::CharDevice) {
            return Err(errno::ENOTTY);
        }
        match &self.file {
            File::Stream(tty) => tty.inode().ioctl(request, argp),
            _ => Err(errno::ENOTTY),
        }
    }

    pub fn set_close_on_exec(&mut self, set: bool) {
        self.flags.set(OpenFlags::CLOEXEC, set);
    }

    pub fn flags(&self) -> OpenFlags {
        self.flags
    }

    pub fn debug_name(&self) -> &str {
        match &self.file {
            File::Pipe(_) => "<pipe>",
            File::Dir(dir) => dir.dentry.name(),
            File::Seekable(seekable) => seekable.dentry.name(),
            File::Stream(stream) => stream.name(),
        }
    }
}

impl Deref for FileDescriptor {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
