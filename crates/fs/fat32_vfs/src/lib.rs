#![no_std]
#![feature(coroutines, iter_from_coroutine)]

#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

// TODO: [mid] 现在整个磁盘文件系统都没有将修改同步回磁盘

mod dir;
mod file;

use defines::{
    error::{errno, KResult},
    fs::StatFsFlags,
};
use ecow::EcoString;
use fat32::{BiosParameterBlock, FileAllocTable, BOOT_SECTOR_ID, SECTOR_SIZE};
use hal::block_device::BlockDevice;
use libkernel::fs::{dentry::DEntryDir, inode::DynDirInodeCoercion, FileSystem};
use triomphe::Arc;
use unsize::CoerceUnsize;

use crate::dir::FatDir;

pub const FS_TYPE: &str = "vfat";

pub fn new_fat32_fs(
    block_device: &'static dyn BlockDevice,
    name: EcoString,
    device_path: EcoString,
    flags: StatFsFlags,
) -> KResult<FileSystem> {
    let _enter = debug_span!("fat32_fs_init").entered();
    let bpb = {
        let _enter = trace_span!("fat_bpb").entered();
        let mut buf = [0; SECTOR_SIZE];
        block_device.read_block(BOOT_SECTOR_ID, &mut buf);
        BiosParameterBlock::new(&buf)?
    };
    if !is_valid_fat32_bpb(&bpb) {
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
        fs_type: FS_TYPE,
        mounted_dentry: None,
        mount_point,
        flags,
    })
}

fn is_valid_fat32_bpb(bpb: &BiosParameterBlock) -> bool {
    // FAT32 基础字段检查：几何参数、计数字段、FAT32 专属字段
    if bpb.sector_size as usize != SECTOR_SIZE
        || bpb.sector_per_cluster == 0
        || !bpb.sector_per_cluster.is_power_of_two()
        || bpb.sector_per_cluster > 128
        || bpb.reserved_sector_count == 0
        || bpb.fat_count == 0
        || bpb.fat32_length == 0
        || bpb.total_sector_count == 0
        || bpb._root_entry_count != 0
        || bpb._sector_count != 0
        || bpb._fat_length != 0
        || bpb._version != 0
        || bpb.info_sector >= bpb.reserved_sector_count
    {
        return false;
    }

    // 先按 BPB 推导数据区范围，防止出现负数/越界布局
    let fat_start_sector = bpb.reserved_sector_count as u32;
    let fat_sectors = bpb.fat_count as u32 * bpb.fat32_length;
    let data_start_sector = fat_start_sector + fat_sectors;
    if data_start_sector >= bpb.total_sector_count {
        return false;
    }

    // 按 CountOfClusters 判定 FAT 子类型，而不是用总扇区数
    let data_sectors = bpb.total_sector_count - data_start_sector;
    let count_of_clusters = data_sectors / bpb.sector_per_cluster as u32;
    if count_of_clusters < 65_526 {
        return false;
    }

    // 根目录起始簇必须落在有效簇号范围内
    let max_valid_cluster = count_of_clusters + 1;
    if bpb.root_cluster < 2 || bpb.root_cluster > max_valid_cluster {
        return false;
    }

    true
}
