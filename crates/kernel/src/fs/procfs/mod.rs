mod mounts;

use compact_str::CompactString;
use defines::{error::KResult, fs::StatFsFlags};
use mounts::MountsInode;
use triomphe::Arc;
use unsize::CoerceUnsize;

use super::{
    inode::{
        DirInodeBackend, DynBytesInodeCoercion, DynDirInode, DynDirInodeCoercion, DynInode,
        InodeMeta,
    },
    tmpfs, DEntry, DEntryBytes, DEntryDir, DynBytesInode, FileSystem, InodeMode,
};
use crate::time;

pub fn new_proc_fs(
    parent: Arc<DEntryDir>,
    name: CompactString,
    device_path: CompactString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let fs = tmpfs::new_tmp_fs(parent, name, device_path, flags)?;
    {
        let mut children = fs.root_dentry.lock_children();
        let child = Arc::new(MountsInode::new()).unsize(DynBytesInodeCoercion!());
        let child = DEntry::Bytes(Arc::new(DEntryBytes::new(
            Arc::clone(&fs.root_dentry),
            CompactString::from_static_str("mounts"),
            child,
        )));
        children.insert(CompactString::from_static_str("mounts"), child);
    }
    Ok(fs)
}
