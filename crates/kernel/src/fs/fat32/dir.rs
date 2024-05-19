use compact_str::{CompactString, ToCompactString};
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
        dentry::{DEntry, DEntryDir, DEntryPaged},
        fat32::{dir_entry::DirEntryBuilderResult, file::FatFile},
        inode::{
            DirInodeBackend, DynDirInode, DynDirInodeCoercion, DynInode, DynPagedInode,
            DynPagedInodeCoercion, Inode, InodeMeta, InodeMode,
        },
    },
    hart::local_hart,
    time,
};

pub struct FatDir {
    clusters: RwLock<SmallVec<[u32; 4]>>,
    fat: Arc<FileAllocTable>,
    /// 记录目录的创建时间，会同步到磁盘中
    create_time: Option<TimeSpec>,
}

impl FatDir {
    pub fn new_root(fat: Arc<FileAllocTable>, first_root_cluster_id: u32) -> Inode<Self> {
        debug!("init root dir");
        assert!(first_root_cluster_id >= 2);
        let clusters = fat
            .cluster_chain(first_root_cluster_id)
            .collect::<SmallVec<_>>();
        let root_dir = Self {
            clusters: RwLock::new(clusters),
            fat,
            create_time: None,
        };
        let meta = InodeMeta::new(InodeMode::Dir, CompactString::from_static_str("/"));
        meta.lock_inner_with(|inner| {
            inner.data_len = root_dir.disk_space();
        });
        Inode::new(meta, root_dir)
    }

    pub fn from_dir_entry(fat: Arc<FileAllocTable>, mut dir_entry: DirEntry) -> Inode<Self> {
        debug_assert!(dir_entry.is_dir());
        let meta = InodeMeta::new(InodeMode::Dir, dir_entry.take_name());
        let fat_dir = Self {
            clusters: RwLock::new(fat.cluster_chain(dir_entry.first_cluster_id()).collect()),
            fat,
            create_time: None,
        };
        meta.lock_inner_with(|inner| {
            inner.data_len = fat_dir.disk_space();
            inner.access_time = dir_entry.access_time();
            // inode 中并不存储创建时间，而 fat32 并不单独记录文件元数据改变时间
            // 此处将 fat32 的创建时间存放在 inode 的元数据改变时间中
            // NOTE: 同步时不覆盖创建时间
            inner.change_time = dir_entry.create_time();
            inner.modify_time = dir_entry.modify_time();
        });
        Inode::new(meta, fat_dir)
    }

    fn create(fat: Arc<FileAllocTable>, name: &str) -> KResult<Inode<Self>> {
        let allocated_cluster = fat.alloc_cluster(None).ok_or(errno::ENOSPC)?;
        let meta = InodeMeta::new(InodeMode::Dir, name.to_compact_string());
        let curr_time = TimeSpec::from(time::curr_time());
        let fat_dir = Self {
            clusters: RwLock::new(smallvec![allocated_cluster]),
            fat,
            create_time: Some(curr_time),
        };
        meta.lock_inner_with(|inner| {
            inner.data_len = fat_dir.disk_space();
            inner.access_time = TimeSpec::from(time::curr_time());
            inner.change_time = inner.access_time;
            inner.modify_time = inner.access_time;
        });
        Ok(Inode::new(meta, fat_dir))
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

impl DirInodeBackend for FatDir {
    fn lookup(&self, name: &str) -> Option<DynInode> {
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
                return Some(DynInode::Paged(
                    Arc::new(fat_file).unsize(DynPagedInodeCoercion!()),
                ));
            }
        }
        None
    }

    fn mkdir(&self, name: &str) -> KResult<Arc<DynDirInode>> {
        let fat_dir = FatDir::create(Arc::clone(&self.fat), name)?;
        Ok(Arc::new(fat_dir).unsize(DynDirInodeCoercion!()))
    }

    fn mknod(&self, name: &str, mode: InodeMode) -> KResult<Arc<DynPagedInode>> {
        match mode {
            InodeMode::Regular => {}
            InodeMode::Dir | InodeMode::SymbolLink => unreachable!(),
            _ => todo!("[mid] impl mknod for non-regular mode"),
        }
        let fat_file = FatFile::create(Arc::clone(&self.fat), name)?;
        Ok(Arc::new(fat_file).unsize(DynPagedInodeCoercion!()))
    }

    fn unlink(&self, name: &str) -> KResult<()> {
        todo!("[mid] impl unlink for fat32");
    }

    fn read_dir(&self, parent: &Arc<DEntryDir>) -> KResult<()> {
        debug!("fat32 read dir");
        let mut children = parent.lock_children();
        for dir_entry in self.dir_entry_iter(&self.clusters.read()) {
            let Ok(dir_entry) = dir_entry else {
                continue;
            };

            if let Some(child_entry) = children.get(dir_entry.name()) {
                // 该目录项实际存在，因此不可能为 None
                assert!(child_entry.is_some());
                continue;
            }

            let new_dentry = if dir_entry.is_dir() {
                let fat_dir = FatDir::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                DEntry::Dir(Arc::new(DEntryDir::new(
                    Some(Arc::clone(parent)),
                    Arc::new(fat_dir).unsize(DynDirInodeCoercion!()),
                )))
            } else {
                let fat_file = FatFile::from_dir_entry(Arc::clone(&self.fat), dir_entry);
                DEntry::Paged(DEntryPaged::new(
                    Arc::clone(parent),
                    Arc::new(fat_file).unsize(DynPagedInodeCoercion!()),
                ))
            };
            let name = new_dentry.meta().name().to_compact_string();
            children.insert(name, Some(new_dentry));
        }
        Ok(())
    }

    fn disk_space(&self) -> usize {
        self.clusters.read().len() * self.fat.sector_per_cluster() as usize * SECTOR_SIZE
    }
}
