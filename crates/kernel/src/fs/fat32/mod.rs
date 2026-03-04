//! fat32 文件系统的实现。
//!
//! 可以参考：
//! - <https://wiki.osdev.org/FAT/>
//! - <https://www.win.tue.nl/~aeb/linux/fs/fat/fat-1.html/>
//! - <https://github.com/rafalh/rust-fatfs/>

// TODO: [mid] 现在整个磁盘文件系统都没有将修改同步回磁盘

mod dir;
mod file;

use ::fat32::{BiosParameterBlock, BlockDevice, FileAllocTable, BOOT_SECTOR_ID, SECTOR_SIZE};
use defines::{
    error::{errno, KResult},
    fs::StatFsFlags,
};
use ecow::EcoString;
use triomphe::Arc;
use unsize::CoerceUnsize;

use super::FileSystem;
use crate::{
    drivers::qemu_block::DiskDriver,
    fs::{dentry::DEntryDir, fat32::dir::FatDir, inode::DynDirInodeCoercion},
};

impl BlockDevice for DiskDriver {
    fn read_block(&self, block_id: usize, buf: &mut [u8; SECTOR_SIZE]) {
        self.read_blocks(block_id, buf);
    }

    fn read_block_cached(&self, block_id: usize, buf: &mut [u8; SECTOR_SIZE]) {
        self.read_blocks_cached(block_id, buf);
    }
}

pub fn new_fat32_fs(
    block_device: &'static DiskDriver,
    name: EcoString,
    device_path: EcoString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let _enter = debug_span!("fat32_fs_init").entered();
    let bpb = {
        let _enter = trace_span!("fat_bpb").entered();
        let mut buf = [0; SECTOR_SIZE];
        block_device.read_block(BOOT_SECTOR_ID, &mut buf);
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
    let root_dentry = Arc::new(DEntryDir::new(None, name, root_dir));
    let mount_point = root_dentry.path();
    Ok(FileSystem {
        root_dentry,
        device_path,
        fs_type: crate::fs::FileSystemType::VFat,
        mounted_dentry: None,
        mount_point,
        flags,
    })
}
