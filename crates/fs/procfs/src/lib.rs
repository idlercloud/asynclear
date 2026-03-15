#![no_std]

#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

mod meminfo;
mod mounts;

use defines::{error::KResult, fs::StatFsFlags};
use ecow::EcoString;
use libkernel::fs::{
    dentry::{DEntry, DEntryBytes, DEntryDir},
    inode::{DynBytesInode, DynBytesInodeCoercion},
    FileSystem,
};
use meminfo::MeminfoInode;
use mounts::MountsInode;
use triomphe::Arc;
use unsize::CoerceUnsize;

pub const FS_TYPE: &str = "proc";

pub fn new_proc_fs(
    parent: Arc<DEntryDir>,
    name: EcoString,
    device_path: EcoString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let mut fs = tmpfs::new_tmp_fs(parent, name, device_path, flags)?;
    fs.fs_type = FS_TYPE;
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

        add_child("mounts", Arc::new(MountsInode::new()).unsize(DynBytesInodeCoercion!()));
        add_child(
            "meminfo",
            Arc::new(MeminfoInode::new()).unsize(DynBytesInodeCoercion!()),
        );
    }
    Ok(fs)
}
