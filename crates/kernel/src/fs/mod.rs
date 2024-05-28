// FIXME: 完整实现 fs 模块并去除 `#![allow(unused)]`
#![allow(unused)]

mod dentry;
mod fat32;
mod file;
mod inode;
mod page_cache;
mod pipe;
mod stdio;
mod tmpfs;

use alloc::{string::String, vec::Vec};
use core::str::FromStr;

use cervine::Cow;
use compact_str::{CompactString, ToCompactString};
use defines::{
    error::{errno, KResult},
    fs::{MountFlags, Stat, StatMode, UnmountFlags, AT_FDCWD},
};
use hashbrown::HashMap;
use klocks::{Lazy, SpinMutex};
use triomphe::Arc;

use self::inode::InodeMeta;
pub use self::{
    dentry::{DEntry, DEntryBytes, DEntryDir},
    file::{DirFile, FdTable, File, FileDescriptor, SeekFrom, SeekableFile},
    inode::{DynBytesInode, InodeMode},
    pipe::make_pipe,
};
use crate::{
    drivers::qemu_block::{BLOCK_DEVICE, BLOCK_SIZE},
    hart::local_hart,
    uart_console::println,
};

pub fn init() {
    Lazy::force(&VFS);
}

pub struct VirtFileSystem {
    root_dir: Arc<DEntryDir>,
    mount_table: SpinMutex<HashMap<DEntry, FileSystem>>,
}

impl VirtFileSystem {
    pub fn root_dir(&self) -> &Arc<DEntryDir> {
        &self.root_dir
    }

    pub fn mount(
        &self,
        mount_point: &str,
        device_path: &str,
        fs_type: FileSystemType,
        flags: MountFlags,
    ) -> KResult<()> {
        debug!("mount {device_path} under {mount_point}, fs_type: {fs_type:?}, flags: {flags:?}",);
        let p2i = resolve_path_with_dir_fd(AT_FDCWD, mount_point)?;
        let dentry = p2i
            .dir
            .lookup(Cow::Borrowed(&p2i.last_component))
            .ok_or(errno::ENOENT)?;

        let mut mount_table = self.mount_table.lock();

        if mount_table.contains_key(&dentry) {
            // 暂时不支持挂载到已有挂载的挂载点上
            error!("cover mount fs not supported yet");
            return Err(errno::EBUSY);
        }
        let parent = match &dentry {
            DEntry::Dir(dir) => match dir.parent() {
                Some(parent) => parent,
                None => todo!("[low] mount under root dir"),
            },
            DEntry::Bytes(bytes) => bytes.parent(),
        };

        let fs = {
            let mut children = parent.lock_children();

            let name = dentry.name();

            // TODO: 暂时是放了一个 tmpfs 进去
            let mut fs = tmpfs::new_tmp_fs(
                Arc::clone(parent),
                name.to_compact_string(),
                device_path.to_compact_string(),
            )?;

            if let Some(Some(covered_dentry)) = children.insert(
                name.to_compact_string(),
                Some(DEntry::Dir(Arc::clone(&fs.root_dentry))),
            ) {
                fs.mounted_dentry = Some(covered_dentry);
            }

            fs
        };
        mount_table.insert(DEntry::Dir(Arc::clone(&fs.root_dentry)), fs);
        Ok(())
    }

    pub fn unmount(&self, mount_point: &str, flags: UnmountFlags) -> KResult<()> {
        debug!("mount {mount_point}, flags: {flags:?}");
        let p2i = resolve_path_with_dir_fd(AT_FDCWD, mount_point)?;
        let dentry = p2i
            .dir
            .lookup(Cow::Borrowed(&p2i.last_component))
            .ok_or(errno::ENOENT)?;

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
            parent.lock_children().insert(
                mounted_dentry.name().to_compact_string(),
                Some(mounted_dentry),
            );
        }

        Ok(())
    }
}

pub static VFS: Lazy<VirtFileSystem> = Lazy::new(|| {
    debug!("Init vfs");
    let root_fs = fat32::new_fat32_fs(
        &BLOCK_DEVICE,
        CompactString::from_static_str("/"),
        CompactString::from_static_str("/dev/mmcblk0"),
    )
    .expect("root_fs init failed");

    root_fs
        .root_dentry
        .read_dir()
        .expect("read root dir failed");
    {
        let children = root_fs.root_dentry.lock_children();
        let mut curr_col = 0;
        let mut output = String::with_capacity(128);
        for name in children.keys() {
            let mut this_end = curr_col + name.len();
            // 当前行超过硬上限了，且至少输出了一个名字，因此换行
            if this_end > 120 && curr_col != 0 {
                curr_col = 0;
                this_end = name.len();
                output.push('\n');
            }

            output.push_str(name);
            // 当前行达到硬上限，但一个名字都没输出；或者达到了软上限。输出后立刻换行
            if this_end >= 120 && curr_col == 0 || this_end >= 80 {
                output.push('\n');
                curr_col = 0;
            }
            // 当前行未达到上限，继续尝试在当前行输出
            else {
                output.push_str("  ");
                curr_col = this_end + 2;
            }
        }
        println!("{output}");
    }

    let root_dir = Arc::clone(&root_fs.root_dentry);
    let mount_table = HashMap::from([(DEntry::Dir(Arc::clone(&root_dir)), root_fs)]);
    VirtFileSystem {
        root_dir,
        mount_table: SpinMutex::new(mount_table),
    }
});

pub struct FileSystem {
    root_dentry: Arc<DEntryDir>,
    device_path: CompactString,
    fs_type: FileSystemType,
    mounted_dentry: Option<DEntry>,
}

#[derive(Debug)]
pub enum FileSystemType {
    VFat,
    Tmpfs,
}

impl FromStr for FileSystemType {
    type Err = defines::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "vfat" => Ok(FileSystemType::VFat),
            "tmpfs" => Ok(FileSystemType::Tmpfs),
            _ => Err(errno::ENODEV),
        }
    }
}

/// 类似于 linux 的 `struct nameidata`，存放 path walk 的结果。
///
/// 也就是路径最后一个 component 和前面的其他部分解析得到的目录 dentry
pub struct PathToInode {
    pub dir: Arc<DEntryDir>,
    pub last_component: CompactString,
}

pub fn resolve_path_with_dir_fd(dir_fd: usize, path: &str) -> KResult<PathToInode> {
    let start_dir;
    // 绝对路径则忽视 fd
    if path.starts_with('/') {
        start_dir = Arc::clone(VFS.root_dir());
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

pub fn path_walk(start_dir: Arc<DEntryDir>, path: &str) -> KResult<PathToInode> {
    debug!("walk path: {path}, from {}", start_dir.name());
    let mut split = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .skip_while(|c| c.is_empty());

    let mut ret = PathToInode {
        dir: start_dir,
        last_component: CompactString::from_static_str("."),
    };

    let Some(mut curr_component) = split.next() else {
        return Ok(ret);
    };

    for next_component in split {
        match ret.dir.lookup(Cow::Borrowed(curr_component)) {
            Some(DEntry::Dir(next_dir)) => ret.dir = next_dir,
            Some(_) => return Err(errno::ENOTDIR),
            None => return Err(errno::ENOENT),
        }
        curr_component = next_component;
    }
    ret.last_component = curr_component.to_compact_string();
    Ok(ret)
}

pub fn find_file(path: &str) -> KResult<DEntry> {
    let p2i = resolve_path_with_dir_fd(AT_FDCWD, path)?;
    p2i.dir
        .lookup(Cow::Owned(p2i.last_component))
        .ok_or(errno::ENOENT)
}

pub fn read_file(file: &Arc<DynBytesInode>) -> KResult<Vec<u8>> {
    // NOTE: 这里其实可能有 race？读写同时发生时 `data_len` 可能会比较微妙
    let data_len = file.meta().lock_inner_with(|inner| inner.data_len as usize);
    let mut ret = Vec::with_capacity(data_len);
    let buf = unsafe { core::slice::from_raw_parts_mut(ret.as_mut_ptr(), data_len) };
    let len = file.read_at(buf, 0)?;
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
