// FIXME: 完整实现 fs 模块并去除 `#![allow(unused)]`
#![allow(unused)]

mod dentry;
mod fat32;
mod file;
mod inode;
mod page_cache;
mod stdio;

use alloc::{collections::BTreeMap, vec, vec::Vec};

use compact_str::{CompactString, ToCompactString};
use defines::error::{errno, KResult};
pub use dentry::{DEntry, DEntryDir, DEntryPaged};
pub use file::{FdTable, File, FileDescriptor, OpenFlags, PagedFile};
use klocks::{Lazy, SpinNoIrqMutex};
use triomphe::Arc;

use crate::{drivers::qemu_block::BLOCK_DEVICE, uart_console::println};

pub fn init() {
    Lazy::force(&VFS);
}

pub struct VirtFileSystem {
    root_dir: Arc<DEntryDir>,
    mount_table: SpinNoIrqMutex<BTreeMap<CompactString, FileSystem>>,
}

impl VirtFileSystem {
    pub fn root_dir(&self) -> &Arc<DEntryDir> {
        &self.root_dir
    }

    // pub fn mount(&self, mount_point: &str, device_path: CompactString, fs_type: FileSystemType) {}
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
        for name in children.keys() {
            println!("{name}");
        }
    }

    let root_dir = Arc::clone(&root_fs.root_dentry);
    let mount_table = BTreeMap::from([(CompactString::from_static_str("/"), root_fs)]);
    VirtFileSystem {
        root_dir,
        mount_table: SpinNoIrqMutex::new(mount_table),
    }
});

pub struct FileSystem {
    root_dentry: Arc<DEntryDir>,
    device_path: CompactString,
    fs_type: FileSystemType,
    mounted_dentry: Option<DEntry>,
}

pub enum FileSystemType {
    Fat32,
}

/// 类似于 linux 的 `struct nameidata`，存放 path walk 的结果。
///
/// 也就是路径最后一个 component 和前面的其他部分解析得到的目录 dentry
pub struct PathToInode {
    pub dir: Arc<DEntryDir>,
    pub last_component: CompactString,
}

/// 分类讨论：
///
/// 1. "/"，如 open("/")，返回 `root_dir()`, `.`。
/// 2. "/xxx"，返回 `dir`, `Some(last_component)`
/// 3. 某个中间的 component 不存在，则返回 ENOENT
/// 4. 某个中间的 component 存在但不是目录（即使后面跟的是 `..` 或 `.`），则返回
///    ENOTDIR
pub fn path_walk(start_dir: Arc<DEntryDir>, path: &str) -> KResult<PathToInode> {
    debug!(
        "walk path: {path}, from {}",
        start_dir.inode().meta().name()
    );
    let mut split = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/');

    let mut ret = PathToInode {
        dir: start_dir,
        last_component: CompactString::from_static_str("."),
    };

    let Some(mut curr_component) = split.next() else {
        return Ok(ret);
    };
    let Some(mut next_component) = split.next() else {
        ret.last_component = curr_component.to_compact_string();
        return Ok(ret);
    };

    loop {
        debug!("component: {curr_component}");

        if let Some(new_component) = split.next() {
            // 当前是一个中间的 component
            let maybe_next = ret.dir.lookup(curr_component.to_compact_string());
            curr_component = next_component;
            next_component = new_component;
            match maybe_next {
                Some(DEntry::Dir(next_dir)) => ret.dir = next_dir,
                Some(_) => return Err(errno::ENOTDIR),
                None => return Err(errno::ENOENT),
            }
        } else {
            // 当前是最后一个 component
            ret.last_component = curr_component.to_compact_string();
            return Ok(ret);
        }
    }
}

pub fn find_file(start_dir: Arc<DEntryDir>, path: &str) -> KResult<DEntry> {
    let p2i = path_walk(start_dir, path)?;
    p2i.dir.lookup(p2i.last_component).ok_or(errno::ENOENT)
}

pub fn read_file(file: &DEntryPaged) -> KResult<Vec<u8>> {
    // NOTE: 这里其实可能有 race？读写同时发生时 `data_len` 可能会比较微妙
    let inner = &file.inode().inner;
    // TODO: 这里其实可以说不定可以打点 unsafe 体操避免初始化的开销
    let mut ret = vec![0; inner.data_len()];
    let len = inner.read_at(file.inode().meta(), &mut ret, 0)?;
    ret.truncate(len);
    Ok(ret)
}
