#![no_std]

use defines::{error::KResult, fs::StatFsFlags};
use ecow::EcoString;
use executor::time;
use libkernel::fs::{
    dentry::DEntryDir,
    inode::{DirInodeBackend, DynBytesInode, DynDirInode, DynDirInodeCoercion, DynInode, InodeMeta, InodeMode},
    FileSystem,
};
use triomphe::Arc;
use unsize::CoerceUnsize;

// TODO: [mid] 完善 tmpfs

pub const FS_TYPE: &str = "tmpfs";

pub fn new_tmp_fs(
    parent: Arc<DEntryDir>,
    name: EcoString,
    device_path: EcoString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let root_dir = Arc::new(TmpDir::new()).unsize(DynDirInodeCoercion!());
    let root_dentry = Arc::new(DEntryDir::new(Some(parent), name, root_dir));
    let mount_point = root_dentry.path();

    Ok(FileSystem {
        root_dentry,
        device_path,
        fs_type: FS_TYPE,
        mounted_dentry: None,
        mount_point,
        flags,
    })
}

pub struct TmpDir {
    meta: InodeMeta,
}

impl TmpDir {
    pub fn new() -> Self {
        let mut meta = InodeMeta::new(InodeMode::Dir);
        let meta_inner = meta.get_inner_mut();
        let curr_time = time::curr_time_spec();
        meta_inner.access_time = curr_time;
        meta_inner.change_time = curr_time;
        meta_inner.modify_time = curr_time;
        Self { meta }
    }
}

impl Default for TmpDir {
    fn default() -> Self {
        Self::new()
    }
}

// TmpDir 的主要机制都是靠 DEntryDir 的，所以这里不需要做太多事儿
impl DirInodeBackend for TmpDir {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn lookup(&self, _name: &str) -> Option<DynInode> {
        None
    }

    fn mkdir(&self, _name: &str) -> KResult<Arc<DynDirInode>> {
        Ok(Arc::new(Self::new()).unsize(DynDirInodeCoercion!()))
    }

    fn mknod(&self, _name: &str, mode: InodeMode) -> KResult<Arc<DynBytesInode>> {
        debug_assert_ne!(mode, InodeMode::Dir);
        todo!("[low] impl mknod for tmpfs");
    }

    fn unlink(&self, _name: &str) -> KResult<()> {
        Ok(())
    }

    fn read_dir(&self, _parent: &Arc<DEntryDir>) -> KResult<()> {
        Ok(())
    }

    fn disk_space(&self) -> u64 {
        0
    }
}

#[expect(unused)]
pub struct TmpFile {
    meta: InodeMeta,
}
