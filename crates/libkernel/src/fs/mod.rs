// FIXME: 完整实现 fs 模块并去除 `#![allow(unused)]`
#![allow(unused)]

pub mod dentry;
pub mod file;
pub mod inode;
mod page_cache;
pub mod pipe;

use alloc::{string::String, vec::Vec};
use core::{fmt::Write, str::FromStr};

use anstyle::{AnsiColor, Reset};
use bitflags::Flags;
use defines::{
    error::{errno, KResult},
    fs::{MountFlags, Stat, StatFsFlags, StatMode, UnmountFlags, AT_FDCWD},
};
use derive_more::Display;
use ecow::EcoString;
use hal::block_device::BLOCK_SIZE;
use hashbrown::HashMap;
use klocks::{Lazy, Once, SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use crate::{
    fs::{
        dentry::{DEntry, DEntryDir},
        file::File,
        inode::{DynBytesInode, InodeMeta},
    },
    hart::local_hart,
    memory::ReadBuffer,
};

pub struct VirtFileSystem {
    root_dir: Arc<DEntryDir>,
    mount_table: SpinMutex<HashMap<DEntry, FileSystem>>,
}

static INSTANCE: Once<VirtFileSystem> = Once::new();

impl VirtFileSystem {
    pub fn new(root_dir: Arc<DEntryDir>, mount_table: HashMap<DEntry, FileSystem>) -> Self {
        Self {
            root_dir,
            mount_table: SpinMutex::new(mount_table),
        }
    }

    pub fn init_instance(instance: VirtFileSystem) {
        INSTANCE.call_once(|| instance);
    }

    #[track_caller]
    pub fn instance() -> &'static Self {
        INSTANCE.get().unwrap()
    }

    pub fn root_dir(&self) -> &Arc<DEntryDir> {
        &self.root_dir
    }

    pub fn mount(
        &self,
        mount_point: &str,
        device_path: &str,
        create_fs: impl FnOnce(Arc<DEntryDir>, EcoString, EcoString, StatFsFlags) -> KResult<FileSystem>,
        flags: MountFlags,
    ) -> KResult<()> {
        let p2i = resolve_path_with_dir_fd(AT_FDCWD, mount_point)?;
        // TODO: [low] 不太确定 stat fs flags 是怎么来的，目前是从 mount flags 里截取一部分
        let statfs_flags = StatFsFlags::from_bits_truncate(flags.bits() & 0b1_1101_1111);
        let mut mount_table = self.mount_table.lock();
        // NOTE: linux 要求挂载必须发生在一个存在的目录上，但是我们的实现似乎不需要
        let mounted_dentry;
        let mut name = p2i.last_component.clone();

        if let Some(dentry) = p2i.dir.lookup(&name) {
            if mount_table.contains_key(&dentry) {
                // 暂时不支持挂载到已有挂载的挂载点上
                error!("cover mount fs not supported yet");
                return Err(errno::EBUSY);
            }
            mounted_dentry = Some(dentry);
            if name != "." && name != ".." {
                name = mounted_dentry.as_ref().unwrap().name().clone();
            }
        } else {
            mounted_dentry = None;
        };
        let parent = if let Some(mounted_dentry) = &mounted_dentry {
            match mounted_dentry {
                DEntry::Dir(dir) => match dir.parent() {
                    Some(parent) => Arc::clone(parent),
                    None => todo!("[low] mount under root dir"),
                },
                DEntry::Bytes(bytes) => Arc::clone(bytes.parent()),
            }
        } else {
            p2i.dir
        };

        let mut fs = create_fs(
            Arc::clone(&parent),
            name.clone(),
            EcoString::from(device_path),
            statfs_flags,
        )?;
        {
            let mut children = parent.lock_children();
            children.insert(name, DEntry::Dir(Arc::clone(&fs.root_dentry)));
        };
        fs.mounted_dentry = mounted_dentry;
        mount_table.insert(DEntry::Dir(Arc::clone(&fs.root_dentry)), fs);
        Ok(())
    }

    pub fn unmount(&self, mount_point: &str, flags: UnmountFlags) -> KResult<()> {
        debug!("mount {mount_point}, flags: {flags:?}");
        let p2i = resolve_path_with_dir_fd(AT_FDCWD, mount_point)?;
        let dentry = p2i.dir.lookup(p2i.last_component).ok_or(errno::ENOENT)?;

        let Some(fs) = self.mount_table.lock().remove(&dentry) else {
            return Err(errno::EINVAL);
        };

        if let Some(mounted_dentry) = fs.mounted_dentry {
            let parent = match &dentry {
                DEntry::Dir(dir) => match dir.parent() {
                    Some(parent) => parent,
                    None => todo!("[low] mount under root dir"),
                },
                DEntry::Bytes(bytes) => bytes.parent(),
            };
            let mut children = parent.lock_children();
            let entry = children.get_mut(mounted_dentry.name()).unwrap();
            *entry = mounted_dentry;
        }

        Ok(())
    }

    pub fn mounts_info(&self) -> EcoString {
        let mut ret = EcoString::new();
        let mounts = self.mount_table.lock();
        let mut n_write = 0;
        for (dentry, fs) in mounts.iter() {
            writeln!(
                ret,
                "{} {} {} {} 0 0",
                fs.device_path, fs.mount_point, fs.fs_type, fs.flags
            )
            .expect("should not fail");
            // TODO: [low]。是不是要考虑被覆盖挂载的文件系统？
        }
        ret
    }

    pub fn lock_mount_table(&self) -> SpinMutexGuard<'_, HashMap<DEntry, FileSystem>> {
        self.mount_table.lock()
    }

    // TODO: 下面的函数也许可以改成直接拿锁过的 mount_table，考虑到原子性问题

    pub fn is_mount_root(&self, dentry: &DEntry) -> bool {
        self.mount_table.lock().contains_key(dentry)
    }

    pub fn mounted_root_of(&self, mut dentry: DEntry) -> DEntry {
        let mount_table = self.mount_table.lock();
        loop {
            if mount_table.contains_key(&dentry) {
                return dentry;
            }
            dentry = match dentry {
                DEntry::Dir(dir) => DEntry::Dir(Arc::clone(
                    dir.parent().expect("mount root should be reachable from dentry"),
                )),
                DEntry::Bytes(bytes) => DEntry::Dir(Arc::clone(bytes.parent())),
            };
        }
    }

    pub fn same_mounted_fs(&self, a: DEntry, b: DEntry) -> bool {
        self.mounted_root_of(a) == self.mounted_root_of(b)
    }
}

pub struct FileSystem {
    pub root_dentry: Arc<DEntryDir>,
    pub device_path: EcoString,
    pub fs_type: &'static str,
    pub mounted_dentry: Option<DEntry>,
    pub mount_point: EcoString,
    pub flags: StatFsFlags,
}

/// 类似于 linux 的 `struct nameidata`，存放 path walk 的结果。
///
/// 也就是路径最后一个 component 和前面的其他部分解析得到的目录 dentry
pub struct PathToInode {
    pub dir: Arc<DEntryDir>,
    pub last_type: LastComponentType,
    pub last_component: EcoString,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LastComponentType {
    Normal,
    Dot,
    DotDot,
    Root,
}

/// `path` 不应为空，否则返回 [`errno::ENOENT`]
pub fn resolve_path_with_dir_fd(dir_fd: usize, path: &str) -> KResult<PathToInode> {
    if path.is_empty() {
        return Err(errno::ENOENT);
    }
    let start_dir;
    // 绝对路径则忽视 fd
    if path.starts_with('/') {
        start_dir = Arc::clone(VirtFileSystem::instance().root_dir());
    } else {
        let process = local_hart().curr_process();
        let inner = process.lock_inner();
        if dir_fd == AT_FDCWD {
            start_dir = Arc::clone(&inner.cwd);
        } else if let Some(base) = inner.fd_table.get(dir_fd) {
            // 相对路径名，需要从一个目录开始
            let File::Dir(dir) = &**base else {
                return Err(errno::ENOTDIR);
            };
            start_dir = Arc::clone(dir.dentry());
        } else {
            return Err(errno::EBADF);
        }
    }

    path_walk(start_dir, path)
}

/// `path` 不应为空，否则返回 [`errno::ENOENT`]
pub fn path_walk(start_dir: Arc<DEntryDir>, path: &str) -> KResult<PathToInode> {
    debug!("walk path: {path}, from {}", start_dir.name());

    // 边缘情况：
    // 多个连续 `/` 等价于单个 `/`
    // 因此 `///` 视作根目录，`a//b` 视作 a/b
    let mut split = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .filter(|c| !c.is_empty());

    let mut ret = PathToInode {
        dir: start_dir,
        last_type: LastComponentType::Normal,
        last_component: EcoString::from("."),
    };

    let Some(mut curr_component) = split.next() else {
        ret.last_type = LastComponentType::Root;
        return Ok(ret);
    };

    for next_component in split {
        match ret.dir.lookup(curr_component) {
            Some(DEntry::Dir(next_dir)) => ret.dir = next_dir,
            Some(_) => return Err(errno::ENOTDIR),
            None => return Err(errno::ENOENT),
        }
        curr_component = next_component;
    }
    if curr_component == "." {
        ret.last_type = LastComponentType::Dot;
    } else if curr_component == ".." {
        ret.last_type = LastComponentType::DotDot;
    }
    ret.last_component = EcoString::from(curr_component);
    Ok(ret)
}

pub fn find_file(path: &str) -> KResult<DEntry> {
    let p2i = resolve_path_with_dir_fd(AT_FDCWD, path)?;
    p2i.dir.lookup(p2i.last_component).ok_or(errno::ENOENT)
}

pub async fn read_file(file: &Arc<DynBytesInode>) -> KResult<Vec<u8>> {
    // NOTE: 这里其实可能有 race？读写同时发生时 `data_len` 可能会比较微妙
    let data_len = file.meta().lock_inner_with(|inner| inner.data_len as usize);
    let mut ret = Vec::with_capacity(data_len);
    let buf = unsafe { core::slice::from_raw_parts_mut(ret.as_mut_ptr(), data_len) };
    let len = file.read_at(ReadBuffer::Kernel(buf), 0).await?;
    // SAFETY: `0..len` 在 read_at 中已被初始化
    unsafe { ret.set_len(len) }
    Ok(ret)
}

pub fn stat_from_meta(meta: &InodeMeta) -> Stat {
    let mut stat = Stat::default();
    // TODO: fstat 的 device id 暂时是一个随意的数字
    stat.st_dev = 114514;
    stat.st_ino = meta.ino() as u64;
    stat.st_mode = StatMode::from(meta.mode());
    stat.st_nlink = 1;
    stat.st_uid = 0;
    stat.st_gid = 0;
    stat.st_rdev = 0;
    // TODO: 特殊文件也先填成 BLOCK_SIZE 吧
    stat.st_blksize = BLOCK_SIZE as u32;
    // TODO: 文件有空洞时，可能小于 st_size/512。而且可能实际占用的块数量会更多
    meta.lock_inner_with(|meta_inner| {
        stat.st_size = meta_inner.data_len;
        stat.st_atime = meta_inner.access_time;
        stat.st_mtime = meta_inner.modify_time;
        stat.st_ctime = meta_inner.change_time;
    });
    stat.st_blocks = stat.st_size.div_ceil(stat.st_blksize as u64);
    stat
}
