use compact_str::{CompactString, ToCompactString};
use defines::{error::KResult, misc::TimeSpec};
use triomphe::Arc;
use unsize::CoerceUnsize;

use super::{
    inode::{DirInodeBackend, DynDirInode, DynDirInodeCoercion, DynInode, InodeMeta},
    DEntryDir, DynBytesInode, FileSystem, InodeMode,
};
use crate::time;

pub fn new_tmp_fs(
    parent: Arc<DEntryDir>,
    name: CompactString,
    device_path: CompactString,
) -> KResult<FileSystem> {
    let root_dir = Arc::new(TmpDir::new(name)).unsize(DynDirInodeCoercion!());
    let root_dentry = Arc::new(DEntryDir::new(Some(parent), root_dir));
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
    pub fn new(name: CompactString) -> Self {
        let meta = InodeMeta::new(InodeMode::Dir, name.to_compact_string());
        meta.lock_inner_with(|inner| {
            inner.access_time = TimeSpec::from(time::curr_time());
            inner.change_time = inner.access_time;
            inner.modify_time = inner.access_time;
        });
        Self { meta }
    }
}

impl DirInodeBackend for TmpDir {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn lookup(&self, _name: &str) -> Option<DynInode> {
        None
    }

    fn mkdir(&self, name: &str) -> KResult<Arc<DynDirInode>> {
        Ok(Arc::new(Self::new(name.to_compact_string())).unsize(DynDirInodeCoercion!()))
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
