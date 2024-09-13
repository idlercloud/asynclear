mod meminfo;
mod mounts;

use defines::{error::KResult, fs::StatFsFlags};
use ecow::EcoString;
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
    name: EcoString,
    device_path: EcoString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let fs = tmpfs::new_tmp_fs(parent, name, device_path, flags)?;
    {
        let mut children = fs.root_dentry.lock_children();
        let mut add_child = |name: &'static str, inode: Arc<DynBytesInode>| {
            let name = EcoString::from(name);
            let child = DEntry::Bytes(Arc::new(DEntryBytes::new(
                Arc::clone(&fs.root_dentry),
                name.clone(),
                inode,
            )));
            children.insert(name, child);
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
