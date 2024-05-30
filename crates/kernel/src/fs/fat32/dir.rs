use alloc::collections::btree_map::Entry;

use defines::{
    error::{errno, KResult},
    misc::TimeSpec,
};
use klocks::{RwLock, RwLockReadGuard};
use smallvec::{smallvec, SmallVec};
use triomphe::Arc;
use unsize::CoerceUnsize;

use super::{
    dir_entry::{DirEntry, DirEntryBuilder, DIR_ENTRY_SIZE},
    fat::FileAllocTable,
    SECTOR_SIZE,
};
use crate::{
    drivers::qemu_block::BLOCK_SIZE,
    fs::{
        dentry::{DEntry, DEntryBytes, DEntryDir},
        fat32::{dir_entry::DirEntryBuilderResult, file::FatFile},
        inode::{
            DirInodeBackend, DynBytesInode, DynBytesInodeCoercion, DynDirInode,
            DynDirInodeCoercion, DynInode, InodeMeta, InodeMode,
        },
    },
    hart::local_hart,
    time,
};

pub struct FatDir {
    meta: InodeMeta,
    clusters: RwLock<SmallVec<[u32; 4]>>,
    fat: Arc<FileAllocTable>,
    /// 记录目录的创建时间，会同步到磁盘中
    create_time: Option<TimeSpec>,
}

impl FatDir {
    pub fn new_root(fat: Arc<FileAllocTable>, first_root_cluster_id: u32) -> Self {
        debug!("init root dir");
        assert!(first_root_cluster_id >= 2);
        let clusters = fat
            .cluster_chain(first_root_cluster_id)
            .collect::<SmallVec<_>>();
        let meta = InodeMeta::new(InodeMode::Dir);
        let root_dir = Self {
            meta,
            clusters: RwLock::new(clusters),
            fat,
            create_time: None,
        };
        root_dir.meta.lock_inner_with(|inner| {
            inner.data_len = root_dir.disk_space();
        });
        root_dir
    }

    pub fn from_dir_entry(fat: Arc<FileAllocTable>, dir_entry: DirEntry) -> Self {
        debug_assert!(dir_entry.is_dir());
        let meta = InodeMeta::new(InodeMode::Dir);
        let clusters: SmallVec<[u32; 4]> =
            fat.cluster_chain(dir_entry.first_cluster_id()).collect();
        let data_len = clusters_disk_space(&fat, clusters.len() as u64);
        meta.lock_inner_with(|inner| {
            inner.data_len = data_len;
            inner.access_time = dir_entry.access_time();
            // inode 中并不存储创建时间，而 fat32 并不单独记录文件元数据改变时间
            // 此处将 fat32 的创建时间存放在 inode 的元数据改变时间中
            inner.change_time = dir_entry.create_time();
            inner.modify_time = dir_entry.modify_time();
        });
        Self {
            meta,
            clusters: RwLock::new(clusters),
            fat,
            create_time: None,
        }
    }

    fn create(fat: Arc<FileAllocTable>, name: &str) -> KResult<Self> {
        let allocated_cluster = fat.alloc_cluster(None).ok_or(errno::ENOSPC)?;
        let mut meta = InodeMeta::new(InodeMode::Dir);
        let meta_inner = meta.get_inner_mut();
        meta_inner.data_len = clusters_disk_space(&fat, 1);
        let curr_time = time::curr_time_spec();
        meta_inner.access_time = curr_time;
        meta_inner.change_time = curr_time;
        meta_inner.modify_time = curr_time;
        Ok(Self {
            meta,
            clusters: RwLock::new(smallvec![allocated_cluster]),
            fat,
            create_time: Some(curr_time),
        })
    }

    pub fn dir_entry_iter<'a>(
        &'a self,
        clusters: &'a RwLockReadGuard<'a, SmallVec<[u32; 4]>>,
    ) -> impl Iterator<Item = KResult<DirEntry>> + 'a {
        let mut raw_entry_iter = core::iter::from_coroutine(
            #[coroutine]
            || {
                let mut buf = local_hart().block_buffer.borrow_mut();
                for sector_id in clusters
                    .iter()
                    .flat_map(|&cluster_id| self.fat.cluster_sectors(cluster_id))
                {
                    self.fat
                        .block_device
                        .read_blocks_cached(sector_id as usize, &mut buf);
                    for dentry_index in 0..BLOCK_SIZE / DIR_ENTRY_SIZE {
                        let entry_start = dentry_index * DIR_ENTRY_SIZE;
                        if buf[entry_start] == 0 {
                            return;
                        }
                        yield buf[entry_start..entry_start + DIR_ENTRY_SIZE]
                            .try_into()
                            .unwrap();
                    }
                }
            },
        );

        core::iter::from_fn(move || {
            let entry = raw_entry_iter.next()?;
            let mut builder = match DirEntryBuilder::from_entry(&entry) {
                Ok(DirEntryBuilderResult::Builder(builder)) => builder,
                Ok(DirEntryBuilderResult::Final(ret)) => return Some(Ok(ret)),
                Err(e) => return Some(Err(e)),
            };

            loop {
                let entry = raw_entry_iter.next()?;
                builder = match builder.add_entry(&entry) {
                    Ok(DirEntryBuilderResult::Builder(builder)) => builder,
                    Ok(DirEntryBuilderResult::Final(ret)) => return Some(Ok(ret)),
                    Err(e) => return Some(Err(e)),
                }
            }
        })
    }
}

fn clusters_disk_space(fat: &FileAllocTable, n_cluster: u64) -> u64 {
    n_cluster * fat.sector_per_cluster() as u64 * SECTOR_SIZE as u64
}

impl DirInodeBackend for FatDir {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn lookup(&self, name: &str) -> Option<DynInode> {
        let curr_time = time::curr_time_spec();
        self.meta
            .lock_inner_with(|inner| inner.access_time = curr_time);
        for dir_entry in self.dir_entry_iter(&self.clusters.read()) {
            let Ok(dir_entry) = dir_entry else {
                continue;
            };
            if dir_entry.name() != name {
                continue;
            }

            if dir_entry.is_dir() {
                let fat_dir = FatDir::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                return Some(DynInode::Dir(
                    Arc::new(fat_dir).unsize(DynDirInodeCoercion!()),
                ));
            } else {
                let fat_file = FatFile::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                return Some(DynInode::Bytes(
                    Arc::new(fat_file).unsize(DynBytesInodeCoercion!()),
                ));
            }
        }
        None
    }

    fn mkdir(&self, name: &str) -> KResult<Arc<DynDirInode>> {
        let fat_dir = FatDir::create(Arc::clone(&self.fat), name)?;
        // TODO: [mid] fat32 mkdir 实际写入磁盘
        Ok(Arc::new(fat_dir).unsize(DynDirInodeCoercion!()))
    }

    fn mknod(&self, name: &str, mode: InodeMode) -> KResult<Arc<DynBytesInode>> {
        match mode {
            InodeMode::Regular => {}
            InodeMode::Dir | InodeMode::SymbolLink => unreachable!(),
            _ => todo!("[mid] impl mknod for non-regular mode"),
        }
        let fat_file = FatFile::create(Arc::clone(&self.fat))?;
        // TODO: [mid] fat32 mknod 实际写入磁盘
        Ok(Arc::new(fat_file).unsize(DynBytesInodeCoercion!()))
    }

    fn unlink(&self, name: &str) -> KResult<()> {
        // FIXME: 实现 FatDir 的 `unlink()`
        Ok(())
    }

    fn read_dir(&self, parent: &Arc<DEntryDir>) -> KResult<()> {
        debug!("fat32 read dir");
        let mut children = parent.lock_children();
        for dir_entry in self.dir_entry_iter(&self.clusters.read()) {
            let Ok(mut dir_entry) = dir_entry else {
                continue;
            };

            let Entry::Vacant(vacant) = children.entry(dir_entry.take_name()) else {
                continue;
            };

            let new_dentry = if dir_entry.is_dir() {
                let fat_dir = FatDir::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                DEntry::Dir(Arc::new(DEntryDir::new(
                    Some(Arc::clone(parent)),
                    vacant.key().clone(),
                    Arc::new(fat_dir).unsize(DynDirInodeCoercion!()),
                )))
            } else {
                let fat_file = FatFile::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                DEntry::Bytes(Arc::new(DEntryBytes::new(
                    Arc::clone(parent),
                    vacant.key().clone(),
                    Arc::new(fat_file).unsize(DynBytesInodeCoercion!()),
                )))
            };
            vacant.insert(new_dentry);
        }
        let curr_time = time::curr_time_spec();
        self.meta
            .lock_inner_with(|inner| inner.access_time = curr_time);
        Ok(())
    }

    fn disk_space(&self) -> u64 {
        clusters_disk_space(&self.fat, self.clusters.read().len() as u64)
    }
}
