mod tty;

use compact_str::CompactString;
use defines::error::KResult;
use triomphe::Arc;
use unsize::CoerceUnsize;

use self::tty::TtyInode;
use super::{
    inode::{DynBytesInodeCoercion, DynDirInodeCoercion},
    tmpfs::{self, TmpDir},
    DEntry, DEntryBytes, DEntryDir, FileSystem,
};

pub fn new_dev_fs(
    parent: Arc<DEntryDir>,
    name: CompactString,
    device_path: CompactString,
) -> KResult<FileSystem> {
    let fs = tmpfs::new_tmp_fs(parent, name, device_path)?;
    {
        let mut children = fs.root_dentry.lock_children();
        let child = Arc::new(TtyInode::new()).unsize(DynBytesInodeCoercion!());
        let child = DEntry::Bytes(Arc::new(DEntryBytes::new(
            Arc::clone(&fs.root_dentry),
            CompactString::from_static_str("tty"),
            child,
        )));
        children.insert(CompactString::from_static_str("tty"), child);
    }
    Ok(fs)
}
