use common::config::{PAGE_OFFSET_MASK, PAGE_SIZE, PAGE_SIZE_BITS};
use compact_str::ToCompactString;
use defines::{
    error::{errno, KResult},
    misc::TimeSpec,
};
use klocks::RwLock;
use smallvec::{smallvec, SmallVec};
use triomphe::Arc;

use super::{dir_entry::DirEntry, fat::FileAllocTable, SECTOR_SIZE};
use crate::{
    fs::{
        inode::{BytesInodeBackend, InodeMeta, InodeMode},
        page_cache::PageCache,
    },
    time,
};

pub struct FatFile {
    meta: InodeMeta,
    clusters: RwLock<SmallVec<[u32; 8]>>,
    fat: Arc<FileAllocTable>,
    /// 记录文件的创建时间，会同步到磁盘中
    create_time: Option<TimeSpec>,
}

impl FatFile {
    pub fn from_dir_entry(fat: Arc<FileAllocTable>, mut dir_entry: DirEntry) -> Self {
        debug_assert!(!dir_entry.is_dir());
        let clusters = fat
            .cluster_chain(dir_entry.first_cluster_id())
            .collect::<SmallVec<_>>();
        // 文件的大小显然是不超过它占用的簇的总大小的
        assert!(
            dir_entry.file_size()
                <= clusters.len() as u64 * fat.sector_per_cluster() as u64 * SECTOR_SIZE as u64
        );
        let mut meta = InodeMeta::new(InodeMode::Regular, dir_entry.take_name());
        let meta_inner = meta.get_inner_mut();
        meta_inner.data_len = dir_entry.file_size();
        meta_inner.access_time = dir_entry.access_time();
        // inode 中并不存储创建时间，而 fat32 并不单独记录文件元数据改变时间
        // 此处将 fat32 的创建时间存放在 inode 的元数据改变时间中
        meta_inner.change_time = dir_entry.create_time();
        meta_inner.modify_time = dir_entry.modify_time();
        Self {
            meta,
            clusters: RwLock::new(clusters),
            fat,
            create_time: None,
        }
    }

    pub fn create(fat: Arc<FileAllocTable>, name: &str) -> KResult<Self> {
        let allocated_cluster = fat.alloc_cluster(None).ok_or(errno::ENOSPC)?;
        let meta = InodeMeta::new(InodeMode::Regular, name.to_compact_string());
        let curr_time = time::curr_time_spec();
        meta.lock_inner_with(|inner| {
            inner.access_time = curr_time;
            inner.change_time = curr_time;
            inner.modify_time = curr_time;
        });
        Ok(Self {
            meta,
            clusters: RwLock::new(smallvec![allocated_cluster]),
            fat,
            create_time: Some(curr_time),
        })
    }

    /// 返回对应的簇索引和簇内的扇区索引
    pub fn page_id_to_cluster_pos(&self, page_id: u64) -> (u32, u8) {
        let sector_index = (page_id * SECOTR_COUNT_PER_PAGE as u64) as u32;
        let cluster_index = sector_index / self.fat.sector_per_cluster() as u32;
        let sector_offset = sector_index % self.fat.sector_per_cluster() as u32;
        (cluster_index, sector_offset as u8)
    }
}

const SECOTR_COUNT_PER_PAGE: usize = PAGE_SIZE / SECTOR_SIZE;

impl BytesInodeBackend for FatFile {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn read_inode_at(&self, buf: &mut [u8], offset: u64) -> KResult<usize> {
        let ret = if let Ok(page) = buf.try_into()
            && (offset & PAGE_OFFSET_MASK as u64) == 0
        {
            self.read_page(page, offset >> PAGE_SIZE_BITS as u64)
        } else {
            todo!()
        };
        ret
    }

    fn write_inode_at(&self, buf: &[u8], offset: u64) -> KResult<usize> {
        todo!("[high] impl write_page for FatFile")
    }
}

impl FatFile {
    pub fn read_page(&self, page: &mut [u8; 4096], page_id: u64) -> KResult<usize> {
        let (mut cluster_index, mut sector_offset) = self.page_id_to_cluster_pos(page_id);

        let mut sector_count = 0;
        let clusters = self.clusters.read();
        'ok: loop {
            let cluster_id = clusters[cluster_index as usize];
            let mut sectors = self.fat.cluster_sectors(cluster_id);
            sectors.start += sector_offset as u32;
            for sector_id in sectors {
                self.fat.block_device.read_blocks(
                    sector_id as usize,
                    (&mut page[sector_count * SECTOR_SIZE..(sector_count + 1) * SECTOR_SIZE])
                        .try_into()
                        .unwrap(),
                );
                sector_count += 1;
                if sector_count >= SECOTR_COUNT_PER_PAGE {
                    break 'ok;
                }
            }
            cluster_index += 1;
            if cluster_index as usize >= clusters.len() {
                break 'ok;
            }
            sector_offset = 0;
        }

        Ok(sector_count * SECTOR_SIZE)
    }
}
