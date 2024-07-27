mod meminfo;
mod mounts;

use compact_str::CompactString;
use defines::{error::KResult, fs::StatFsFlags};
use meminfo::MeminfoInode;
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
use crate::{fs::inode::BytesInodeBackend, time};

pub fn new_proc_fs(
    parent: Arc<DEntryDir>,
    name: CompactString,
    device_path: CompactString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let fs = tmpfs::new_tmp_fs(parent, name, device_path, flags)?;
    {
        let mut children = fs.root_dentry.lock_children();
        let mut add_child = |name: &'static str, inode: Arc<DynBytesInode>| {
            let child = DEntry::Bytes(Arc::new(DEntryBytes::new(
                Arc::clone(&fs.root_dentry),
                CompactString::const_new(name),
                inode,
            )));
            children.insert(CompactString::const_new(name), child);
        };
        macro new_inode($ty:ty) {
            Arc::new(<$ty>::new()).unsize(DynBytesInodeCoercion!())
        }

        add_child("mounts", new_inode!(MountsInode));
        add_child("meminfo", new_inode!(MeminfoInode));
    }
    Ok(fs)
}

trait ProcFile {
    fn new() -> Self;
}
