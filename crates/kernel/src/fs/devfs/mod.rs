mod rtc;
mod tty;

use compact_str::CompactString;
use defines::{error::KResult, fs::StatFsFlags};
use rtc::RtcInode;
use triomphe::Arc;
use unsize::CoerceUnsize;

use self::tty::TtyInode;
use super::{
    inode::{DynBytesInodeCoercion, DynDirInodeCoercion},
    tmpfs::{self, TmpDir},
    DEntry, DEntryBytes, DEntryDir, DynBytesInode, FileSystem,
};

pub fn new_dev_fs(
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

        add_child("tty", new_inode!(TtyInode));
        add_child("rtc", new_inode!(RtcInode));
    }
    Ok(fs)
}
