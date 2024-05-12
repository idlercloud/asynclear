use common::config::PAGE_SIZE;
use smallvec::SmallVec;
use triomphe::Arc;

use super::{dir_entry::DirEntry, fat::FileAllocTable, SECTOR_SIZE};
use crate::{
    fs::inode::{Inode, InodeMeta, InodeMode, PagedInode, PagedInodeBackend},
    memory::Frame,
};

pub struct FatFile {
    clusters: SmallVec<[u32; 8]>,
    fat: Arc<FileAllocTable>,
}

impl FatFile {
    pub fn from_dir_entry(
        fat: Arc<FileAllocTable>,
        mut dir_entry: DirEntry,
    ) -> Inode<PagedInode<Self>> {
        debug_assert!(!dir_entry.is_dir());
        let fat_file = Self {
            clusters: fat.cluster_chain(dir_entry.first_cluster_id()).collect(),
            fat,
        };
        let meta = InodeMeta::new(InodeMode::Regular, dir_entry.take_name());
        meta.lock_inner_with(|inner| {
            inner.data_len = dir_entry.file_size();
            inner.access_time = dir_entry.access_time();
            // inode 中并不存储创建时间，而 fat32 并不单独记录文件元数据改变时间
            // 此处将 fat32 的创建时间存放在 inode 的元数据改变时间中
            // NOTE: 同步时不覆盖创建时间
            inner.change_time = dir_entry.create_time();
            inner.modify_time = dir_entry.modify_time();
        });
        // 文件的大小显然是不超过它占用的簇的总大小的
        assert!(
            dir_entry.file_size()
                <= fat_file.clusters.len()
                    * fat_file.fat.sector_per_cluster() as usize
                    * SECTOR_SIZE
        );
        Inode::new(meta, PagedInode::new(fat_file, dir_entry.file_size()))
    }

    /// 返回对应的簇索引和簇内的扇区索引
    pub fn page_id_to_cluster_pos(&self, page_id: usize) -> (u32, u8) {
        let sector_index = (page_id * SECOTR_COUNT_PER_PAGE) as u32;
        let cluster_index = sector_index / self.fat.sector_per_cluster() as u32;
        let sector_offset = sector_index % self.fat.sector_per_cluster() as u32;
        (cluster_index, sector_offset as u8)
    }
}

const SECOTR_COUNT_PER_PAGE: usize = PAGE_SIZE / SECTOR_SIZE;

impl PagedInodeBackend for FatFile {
    fn read_page(&self, frame: &mut Frame, page_id: usize) -> defines::error::KResult<()> {
        let (mut cluster_index, mut sector_offset) = self.page_id_to_cluster_pos(page_id);

        let mut sector_count = 0;
        let bytes = frame.as_page_bytes_mut();
        'ok: loop {
            let cluster_id = self.clusters[cluster_index as usize];
            let mut sectors = self.fat.cluster_sectors(cluster_id);
            sectors.start += sector_offset as u32;
            for sector_id in sectors {
                self.fat.block_device.read_blocks(
                    sector_id as usize,
                    (&mut bytes[sector_count * SECTOR_SIZE..(sector_count + 1) * SECTOR_SIZE])
                        .try_into()
                        .unwrap(),
                );
                sector_count += 1;
                if sector_count >= SECOTR_COUNT_PER_PAGE {
                    break 'ok;
                }
            }
            cluster_index += 1;
            if cluster_index as usize >= self.clusters.len() {
                break 'ok;
            }
            sector_offset = 0;
        }

        Ok(())
    }

    fn write_page(&self, frame: &Frame, page_id: usize) -> defines::error::KResult<()> {
        todo!("[high] impl write_page for FatFile")
    }
}
