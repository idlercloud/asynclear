use alloc::vec::Vec;
use core::ops::Range;

use defines::error::{errno, KResult};
use klocks::{RwLock, SpinMutex};

use super::{bpb::BiosParameterBlock, SECTOR_SIZE};
use crate::{drivers::qemu_block::DiskDriver, hart::local_hart};

const FAT_ENTRY_MASK: u32 = 0x0fff_ffff;
const RESERVED_FAT_ENTRY_COUNT: u32 = 2;
const END_OF_CHAIN: u32 = 0x0fff_ffff;

pub struct FileAllocTable {
    /// Fat 表的数量
    count: u8,
    /// Fat 区域起始的扇区 id
    fat_start_sector_id: u16,
    /// Data 区域起始的扇区 id
    data_start_sector_id: u32,
    /// 每张 Fat 表所用的扇区数
    fat_length: u32,
    sector_per_cluster: u8,
    /// 数据区总共可用的簇数。注意第 0 簇和第 1
    /// 簇不对应数据区中的簇，所以真正的总簇数应该是这个字段加 2
    data_clusters_count: u32,
    alloc_meta: SpinMutex<FatAllocMeta>,
    fat_entries: RwLock<Vec<u32>>,
    pub(super) block_device: &'static DiskDriver,
}

impl FileAllocTable {
    pub fn new(block_device: &'static DiskDriver, bpb: &BiosParameterBlock) -> KResult<Self> {
        let _enter = debug_span!("fat").entered();
        let mut buf = local_hart().block_buffer.borrow_mut();
        block_device.read_blocks(bpb.info_sector as usize, &mut buf);
        let alloc_meta = FatAllocMeta::new(&buf)?;
        let fat_start_sector_id = bpb.reserved_sector_count;
        let fat_length = bpb.fat32_length;

        const FAT_ENTRY_SIZE: usize = core::mem::size_of::<u32>();

        let mut fat_entries =
            Vec::with_capacity(fat_length as usize * SECTOR_SIZE / FAT_ENTRY_SIZE);
        for sector_id in fat_start_sector_id as u32..fat_start_sector_id as u32 + fat_length {
            block_device.read_blocks(sector_id as usize, &mut buf);
            for &entry in buf.array_chunks::<FAT_ENTRY_SIZE>() {
                fat_entries.push(u32::from_le_bytes(entry));
            }
        }
        debug!("fat entries num is {}", fat_entries.len());

        let data_start_sector_id =
            fat_start_sector_id as u32 + bpb.fat_count as u32 * bpb.fat32_length;
        let data_clusters_count =
            (bpb.total_sector_count - data_start_sector_id) / bpb.sector_per_cluster as u32;
        debug!("fat_start_sector_id: {fat_start_sector_id}");

        let ret = Self {
            count: bpb.fat_count,
            fat_start_sector_id,
            data_start_sector_id,
            fat_length,
            sector_per_cluster: bpb.sector_per_cluster,
            data_clusters_count,
            alloc_meta: SpinMutex::new(alloc_meta),
            fat_entries: RwLock::new(fat_entries),
            block_device,
        };
        ret.maintain_alloc_meta();

        Ok(ret)
    }

    fn maintain_alloc_meta(&self) {
        const INVALID_ALLOC_META: u32 = 0xFFFFFFFF;

        let mut meta = self.alloc_meta.lock();
        if meta.free_count == INVALID_ALLOC_META || meta.next_free == INVALID_ALLOC_META {
            meta.free_count = 0;
            meta.next_free = 0;
            let entries = self.fat_entries.read();
            for (cluster_id, entry) in entries
                .iter()
                .enumerate()
                .skip(RESERVED_FAT_ENTRY_COUNT as usize)
            {
                let entry = entry & FAT_ENTRY_MASK;
                if entry == 0 {
                    meta.free_count += 1;
                } else {
                    meta.next_free = cluster_id as u32 + 1;
                }
            }
        }
    }

    pub fn alloc_cluster(&self, prev_cluster: Option<u32>) -> Option<u32> {
        let mut meta = self.alloc_meta.lock();

        let total_cluster_count = self.data_clusters_count + RESERVED_FAT_ENTRY_COUNT;
        let start_cluster_id = if meta.next_free != total_cluster_count {
            meta.next_free
        } else {
            RESERVED_FAT_ENTRY_COUNT
        };

        let mut entries = self.fat_entries.write();

        let find_free_cluster = |start_cluster_id: u32, end_cluster_id: u32| {
            let mut cluster_id = start_cluster_id;
            for &entry in &entries[start_cluster_id as usize..end_cluster_id as usize] {
                let entry = entry & FAT_ENTRY_MASK;
                if entry == 0 {
                    return Some(cluster_id);
                }
                cluster_id += 1;
            }
            None
        };

        let ret = find_free_cluster(start_cluster_id, total_cluster_count).or_else(|| {
            if start_cluster_id > RESERVED_FAT_ENTRY_COUNT {
                find_free_cluster(RESERVED_FAT_ENTRY_COUNT, start_cluster_id)
            } else {
                None
            }
        });

        if let Some(cluster_id) = ret {
            meta.free_count -= 1;
            meta.next_free = cluster_id + 1;
            if let Some(prev_cluster_id) = prev_cluster {
                entries[prev_cluster_id as usize] = cluster_id;
            }
            entries[cluster_id as usize] = END_OF_CHAIN;
        }

        ret
    }

    pub fn cluster_chain(&self, first_cluster_id: u32) -> impl Iterator<Item = u32> + '_ {
        assert!(first_cluster_id >= 2);
        core::iter::from_coroutine(
            #[coroutine]
            move || {
                let entries = self.fat_entries.read();
                let mut curr_cluster_id = first_cluster_id;
                while curr_cluster_id < 0x0fff_fff8 {
                    yield curr_cluster_id;
                    curr_cluster_id = entries[curr_cluster_id as usize];
                }
            },
        )
    }

    pub fn cluster_sectors(&self, cluster_id: u32) -> Range<u32> {
        debug_assert!(cluster_id >= 2);
        let first_sector = self.data_start_sector_id
            + (cluster_id - RESERVED_FAT_ENTRY_COUNT) * self.sector_per_cluster as u32;
        first_sector..first_sector + self.sector_per_cluster as u32
    }

    pub fn sector_per_cluster(&self) -> u8 {
        self.sector_per_cluster
    }
}

struct FatAllocMeta {
    free_count: u32,
    next_free: u32,
}

impl FatAllocMeta {
    pub fn new(info_sector: &[u8; 512]) -> KResult<Self> {
        let lead_sig = u32::from_le_bytes(info_sector[0..4].try_into().unwrap());
        if lead_sig != 0x41615252 {
            return Err(errno::EINVAL);
        };
        let struc_sig = u32::from_le_bytes(info_sector[484..488].try_into().unwrap());
        if struc_sig != 0x61417272 {
            return Err(errno::EINVAL);
        }

        // 剩余簇的数量，如果是 0xffffffff 则表示未知，需要重新计算。并不保证一定精准，但是其值一定不超过磁盘的总簇数
        let free_count = u32::from_le_bytes(info_sector[488..492].try_into().unwrap());
        // 从哪里开始寻找剩余簇的 hint，通常是最后一个被分配出去的簇号 + 1。如果值为 0xffffffff 则表示未知，应当从 2 号簇开始查找
        let next_free = u32::from_le_bytes(info_sector[492..496].try_into().unwrap());

        let trail_sig = u32::from_le_bytes(info_sector[508..512].try_into().unwrap());
        if trail_sig != 0xaa550000 {
            return Err(errno::EINVAL);
        }
        Ok(Self {
            free_count,
            next_free,
        })
    }
}
