use alloc::vec::Vec;
use core::ops::Range;

use defines::error::KResult;
use hal::block_device::BlockDevice;

use crate::{lock, BiosParameterBlock, SECTOR_SIZE};

const FAT_ENTRY_MASK: u32 = 0x0fff_ffff;
const RESERVED_FAT_ENTRY_COUNT: u32 = 2;
const END_OF_CHAIN: u32 = 0x0fff_ffff;

pub struct FileAllocTable {
    /// Data 区域起始的扇区 id
    data_start_sector_id: u32,
    sector_per_cluster: u8,
    /// 数据区总共可用的簇数。
    ///
    /// 注意第 0 簇和第 1 簇不对应数据区中的簇，
    /// 所以真正的总簇数应该是这个字段加 2
    data_clusters_count: u32,
    alloc_meta: lock::SpinMutex<FatAllocMeta>,
    fat_entries: lock::RwLock<Vec<u32>>,
    block_device: &'static dyn BlockDevice,
}

const INVALID_ALLOC_META: u32 = 0xFFFF_FFFF;

impl FileAllocTable {
    pub fn new(block_device: &'static dyn BlockDevice, bpb: &BiosParameterBlock) -> KResult<Self> {
        let _enter = debug_span!("fat").entered();
        // TODO: 引入 percpu block buffer
        let mut buf = [0; SECTOR_SIZE];
        block_device.read_block(bpb.info_sector as usize, &mut buf);
        let alloc_meta = FatAllocMeta::new(&buf);
        let fat_start_sector_id = bpb.reserved_sector_count;
        let fat_length = bpb.fat32_length;
        let data_start_sector_id = fat_start_sector_id as u32 + bpb.fat_count as u32 * fat_length;
        let data_clusters_count = (bpb.total_sector_count - data_start_sector_id) / bpb.sector_per_cluster as u32;

        const FAT_ENTRY_SIZE: usize = core::mem::size_of::<u32>();
        let entries_capacity = fat_length as usize * (SECTOR_SIZE / FAT_ENTRY_SIZE);

        if data_clusters_count > entries_capacity as u32 {
            warn!(
                "Inconsistent meta: data_clusters_count: {data_clusters_count}, entries_capacity: {entries_capacity}"
            );
        }

        let mut fat_entries = Vec::with_capacity(fat_length as usize * SECTOR_SIZE / FAT_ENTRY_SIZE);
        for sector_id in fat_start_sector_id as u32..fat_start_sector_id as u32 + fat_length {
            block_device.read_block(sector_id as usize, &mut buf);
            for entry in buf.iter().copied().array_chunks::<FAT_ENTRY_SIZE>() {
                if fat_entries.len() as u32 >= data_clusters_count + RESERVED_FAT_ENTRY_COUNT {
                    break;
                }
                fat_entries.push(u32::from_le_bytes(entry));
            }
        }

        let ret = Self {
            data_start_sector_id,
            sector_per_cluster: bpb.sector_per_cluster,
            data_clusters_count,
            alloc_meta: lock::SpinMutex::new(alloc_meta),
            fat_entries: lock::RwLock::new(fat_entries),
            block_device,
        };
        ret.maintain_alloc_meta();

        Ok(ret)
    }

    fn maintain_alloc_meta(&self) {
        let mut meta = lock::lock_spin(&self.alloc_meta);
        let total_cluster_count = self.data_clusters_count + RESERVED_FAT_ENTRY_COUNT;
        if meta.free_count > self.data_clusters_count
            || (meta.next_free != INVALID_ALLOC_META
                && !(RESERVED_FAT_ENTRY_COUNT..total_cluster_count).contains(&meta.next_free))
        {
            warn!(
                "invalid fsinfo alloc meta, free_count: {}, next_free: {}, fallback to FAT scan",
                meta.free_count, meta.next_free
            );
            meta.free_count = INVALID_ALLOC_META;
            meta.next_free = INVALID_ALLOC_META;
        }

        if meta.free_count == INVALID_ALLOC_META || meta.next_free == INVALID_ALLOC_META {
            meta.next_free = INVALID_ALLOC_META;
            meta.free_count = 0;
            let entries = lock::read_rw(&self.fat_entries);
            for (cluster_id, entry) in entries.iter().enumerate().skip(RESERVED_FAT_ENTRY_COUNT as usize) {
                let entry = entry & FAT_ENTRY_MASK;
                if entry == 0 {
                    meta.free_count += 1;
                    meta.next_free = meta.next_free.min(cluster_id as u32);
                }
            }
        }

        if meta.next_free == INVALID_ALLOC_META {
            warn!("No spare space");
        }
    }

    /// `prev_cluster` 不为 `None` 时，将新分配的簇链接到 `prev_cluster` 的后面。
    pub fn alloc_cluster(&self, prev_cluster: Option<u32>) -> Option<u32> {
        let mut meta = lock::lock_spin(&self.alloc_meta);

        let total_cluster_count = self.data_clusters_count + RESERVED_FAT_ENTRY_COUNT;
        let start_cluster_id = if (RESERVED_FAT_ENTRY_COUNT..total_cluster_count).contains(&meta.next_free) {
            meta.next_free
        } else {
            RESERVED_FAT_ENTRY_COUNT
        };

        let mut entries = lock::write_rw(&self.fat_entries);

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
            if meta.free_count > 0 {
                meta.free_count -= 1;
            } else {
                warn!("alloc meta free_count underflow, this usually means stale fsinfo");
            }
            meta.next_free = cluster_id + 1;
            if let Some(prev_cluster_id) = prev_cluster {
                entries[prev_cluster_id as usize] = cluster_id;
            }
            entries[cluster_id as usize] = END_OF_CHAIN;
        }

        ret
    }

    pub fn free_clusters(&self, clusters: &[u32], prev_cluster: Option<u32>) {
        if clusters.is_empty() {
            return;
        }
        let mut entries = lock::write_rw(&self.fat_entries);
        if let Some(prev_cluster) = prev_cluster {
            assert_eq!(entries[prev_cluster as usize], clusters[0]);
            entries[prev_cluster as usize] = END_OF_CHAIN;
        }
        for &cluster in clusters {
            entries[cluster as usize] = 0;
        }
        lock::lock_spin(&self.alloc_meta).free_count += clusters.len() as u32;
    }

    pub fn cluster_chain(&self, first_cluster_id: u32) -> impl Iterator<Item = u32> + '_ {
        core::iter::from_coroutine(
            #[coroutine]
            move || {
                if first_cluster_id < 2 {
                    return;
                }
                let entries = lock::read_rw(&self.fat_entries);
                let mut curr_cluster_id = first_cluster_id;
                while curr_cluster_id < 0x0fff_fff8 {
                    yield curr_cluster_id;
                    // TODO: 遇到 bad/reserved/越界簇时应报错并使文件系统重新挂载为只读。
                    curr_cluster_id = entries[curr_cluster_id as usize] & FAT_ENTRY_MASK;
                }
            },
        )
    }

    pub fn cluster_sectors(&self, cluster_id: u32) -> Range<u32> {
        debug_assert!(cluster_id >= 2);
        let first_sector =
            self.data_start_sector_id + (cluster_id - RESERVED_FAT_ENTRY_COUNT) * self.sector_per_cluster as u32;
        first_sector..first_sector + self.sector_per_cluster as u32
    }

    pub fn sector_per_cluster(&self) -> u8 {
        self.sector_per_cluster
    }

    pub fn bytes_per_cluster(&self) -> u64 {
        self.sector_per_cluster as u64 * SECTOR_SIZE as u64
    }

    pub fn block_device(&self) -> &'static dyn BlockDevice {
        self.block_device
    }
}

struct FatAllocMeta {
    free_count: u32,
    next_free: u32,
}

impl FatAllocMeta {
    pub fn new(info_sector: &[u8; 512]) -> Self {
        let lead_sig = u32::from_le_bytes(info_sector[0..4].try_into().unwrap());
        if lead_sig != 0x4161_5252 {
            warn!("invalid fsinfo lead signature: {lead_sig:#x}, fallback to FAT scan");
            return Self::invalid();
        };
        let struc_sig = u32::from_le_bytes(info_sector[484..488].try_into().unwrap());
        if struc_sig != 0x6141_7272 {
            warn!("invalid fsinfo structure signature: {struc_sig:#x}, fallback to FAT scan");
            return Self::invalid();
        }

        // 剩余簇的数量，如果是 0xffffffff 则表示未知，需要重新计算。并不保证一定精准，但是其值一定不超过磁盘的总簇数
        let free_count = u32::from_le_bytes(info_sector[488..492].try_into().unwrap());
        // 从哪里开始寻找剩余簇的 hint，通常是最后一个被分配出去的簇号 + 1。如果值为 0xffffffff 则表示未知，应当从 2 号簇开始查找
        let next_free = u32::from_le_bytes(info_sector[492..496].try_into().unwrap());

        let trail_sig = u32::from_le_bytes(info_sector[508..512].try_into().unwrap());
        if trail_sig != 0xaa55_0000 {
            warn!("invalid fsinfo trail signature: {trail_sig:#x}, fallback to FAT scan");
            return Self::invalid();
        }
        Self { free_count, next_free }
    }

    fn invalid() -> Self {
        Self {
            free_count: INVALID_ALLOC_META,
            next_free: INVALID_ALLOC_META,
        }
    }
}
