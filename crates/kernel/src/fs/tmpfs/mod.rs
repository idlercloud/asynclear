use compact_str::{CompactString, ToCompactString};
use defines::{error::KResult, misc::TimeSpec};
use triomphe::Arc;
use unsize::CoerceUnsize;

use super::{
    inode::{DirInodeBackend, DynDirInode, DynDirInodeCoercion, DynInode, InodeMeta},
    DEntryDir, DynBytesInode, FileSystem, InodeMode,
};
use crate::time;

// TODO: [mid] 完善 tmpfs

pub fn new_tmp_fs(
    parent: Arc<DEntryDir>,
    name: CompactString,
    device_path: CompactString,
) -> KResult<FileSystem> {
    let root_dir = Arc::new(TmpDir::new()).unsize(DynDirInodeCoercion!());
    let root_dentry = Arc::new(DEntryDir::new(Some(parent), name, root_dir));
    Ok(FileSystem {
        root_dentry,
        device_path,
        fs_type: crate::fs::FileSystemType::VFat,
        mounted_dentry: None,
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

    fn mknod(&self, name: &str, mode: InodeMode) -> KResult<Arc<DynBytesInode>> {
        todo!()
    }

    fn unlink(&self, name: &str) -> KResult<()> {
        Ok(())
    }

    fn read_dir(&self, _parent: &Arc<DEntryDir>) -> KResult<()> {
        Ok(())
    }

    fn disk_space(&self) -> u64 {
        0
    }
}
