//! fat32 文件系统的实现。
//!
//! 可以参考：
//! - https://wiki.osdev.org/FAT
//! - https://www.win.tue.nl/~aeb/linux/fs/fat/fat-1.html
//! - https://github.com/rafalh/rust-fatfs

mod bpb;
mod dir;
mod dir_entry;
mod fat;
mod file;

use compact_str::CompactString;
use defines::error::{errno, KResult};
use triomphe::Arc;
use unsize::CoerceUnsize;

use crate::{
    drivers::qemu_block::DiskDriver,
    fs::{
        dentry::DEntryDir,
        fat32::{bpb::BiosParameterBlock, dir::FatDir, fat::FileAllocTable},
        inode::DynDirInodeCoercion,
    },
    hart::local_hart,
};

use super::FileSystem;

const SECTOR_SIZE: usize = 512;
const BOOT_SECTOR_ID: usize = 0;

pub fn new_fat32_fs(
    block_device: &'static DiskDriver,
    mount_point: CompactString,
    device_path: CompactString,
) -> KResult<FileSystem> {
    let _enter = debug_span!("fat32_fs_init").entered();
    let bpb = {
        let _enter = debug_span!("fat_bpb").entered();
        let mut buf = local_hart().block_buffer.borrow_mut();
        block_device.read_blocks(BOOT_SECTOR_ID, &mut buf);
        BiosParameterBlock::new(&buf)
    };
    if bpb.sector_size as usize != SECTOR_SIZE
        || bpb.total_sector_count < 65525
        || bpb._root_entry_count != 0
        || bpb._sector_count != 0
        || bpb._fat_length != 0
        || bpb._version != 0
    {
        return Err(errno::EINVAL);
    }

    debug!("init fat");
    let fat = Arc::new(FileAllocTable::new(block_device, &bpb)?);
    let root_dir = Arc::new(FatDir::new_root(fat, bpb.root_cluster)).unsize(DynDirInodeCoercion!());
    let root_dentry = Arc::new(DEntryDir::new(None, root_dir));
    Ok(FileSystem {
        root_dentry,
        device_path,
        fs_type: crate::fs::FileSystemType::Fat32,
        mounted_dentry: None,
    })
}
