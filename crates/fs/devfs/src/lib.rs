#![no_std]

#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

mod rtc;
mod tty;

use defines::{error::KResult, fs::StatFsFlags};
use ecow::EcoString;
use libkernel::fs::{
    dentry::{DEntry, DEntryBytes, DEntryDir},
    inode::{DynBytesInode, DynBytesInodeCoercion},
    FileSystem,
};
use rtc::RtcInode;
use triomphe::Arc;
use unsize::CoerceUnsize;

use self::tty::TtyInode;

pub const FS_TYPE: &str = "devtmpfs";

pub fn new_dev_fs(
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

        add_child("tty", Arc::new(TtyInode::new()).unsize(DynBytesInodeCoercion!()));
        add_child("rtc", Arc::new(RtcInode::new()).unsize(DynBytesInodeCoercion!()));
    }
    Ok(fs)
}
