use core::{ops::Deref, sync::atomic::AtomicUsize};

use atomic::Ordering;
use common::config::{PAGE_OFFSET_MASK, PAGE_SIZE, PAGE_SIZE_BITS};
use compact_str::CompactString;
use defines::{error::KResult, fs::StatMode, misc::TimeSpec};
use klocks::{RwLock, RwLockReadGuard, SpinMutex};
use triomphe::Arc;
use unsize::{CoerceUnsize, Coercion};

use super::{
    dentry::DEntryDir,
    page_cache::{BackedPage, PageCache},
};
use crate::{executor::block_on, fs::page_cache::PageState, time};

static INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

pub type DynDirInode = dyn DirInodeBackend;
pub type DynBytesInode = dyn BytesInodeBackend;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InodeMode {
    Regular,
    Dir,
    SymbolLink,
    Socket,
    Fifo,
    BlockDevice,
    CharDevice,
}

impl From<InodeMode> for StatMode {
    fn from(value: InodeMode) -> Self {
        match value {
            InodeMode::Regular => StatMode::REGULAR,
            InodeMode::Dir => StatMode::DIR,
            InodeMode::SymbolLink => StatMode::SYM_LINK,
            InodeMode::Socket => StatMode::SOCKET,
            InodeMode::Fifo => StatMode::FIFO,
            InodeMode::BlockDevice => StatMode::BLOCK_DEVICE,
            InodeMode::CharDevice => StatMode::CHAR_DEVICE,
        }
    }
}

pub struct InodeMeta {
    /// inode number，在一个文件系统中唯一标识一个 Inode
    ino: usize,
    mode: InodeMode,
    // FIXME: name 应该是属于 DEntry 的属性，不应该放在这里
    name: CompactString,
    inner: SpinMutex<InodeMetaInner>,
}

impl InodeMeta {
    pub fn new(mode: InodeMode, name: CompactString) -> Self {
        Self {
            ino: INODE_NUMBER.fetch_add(1, Ordering::SeqCst),
            mode,
            name,
            inner: SpinMutex::new(InodeMetaInner {
                data_len: 0,
                access_time: TimeSpec::default(),
                modify_time: TimeSpec::default(),
                change_time: TimeSpec::default(),
            }),
        }
    }

    pub fn ino(&self) -> usize {
        self.ino
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mode(&self) -> InodeMode {
        self.mode
    }

    pub fn lock_inner_with<T>(&self, f: impl FnOnce(&mut InodeMetaInner) -> T) -> T {
        f(&mut self.inner.lock())
    }
}

pub struct InodeMetaInner {
    /// 对常规文件来说，是其文件内容大小；对目录来说，是它目录项列表占据的总共块空间；其他情况是 0
    pub data_len: u64,
    /// 上一次访问时间
    pub access_time: TimeSpec,
    /// 上一次修改时间
    pub modify_time: TimeSpec,
    /// 上一次元数据变化时间
    pub change_time: TimeSpec,
}

pub trait DirInodeBackend: Send + Sync {
    fn meta(&self) -> &InodeMeta;
    fn lookup(&self, name: &str) -> Option<DynInode>;
    fn mkdir(&self, name: &str) -> KResult<Arc<DynDirInode>>;
    fn mknod(&self, name: &str, mode: InodeMode) -> KResult<Arc<DynBytesInode>>;
    fn unlink(&self, name: &str) -> KResult<()>;
    fn read_dir(&self, parent: &Arc<DEntryDir>) -> KResult<()>;
    fn disk_space(&self) -> u64;
}

// NOTE: `meta` 的信息其实不知道应该在这里改还是在具体文件系统里改。
// 目前的想法是，信息第一次获取到，比如第一次从磁盘加载、被创建时由具体文件系统来完成

// impl<T: ?Sized + DirInodeBackend> Inode<T> {
//     pub fn lookup(&self, name: &str) -> Option<DynInode> {
//         let ret = self.inner.lookup(name);
//         self.meta
//             .lock_inner_with(|inner| inner.access_time = TimeSpec::from(time::curr_time()));
//         ret
//     }

//     pub fn mkdir(&self, name: &str) -> KResult<Arc<DynDirInode>> {
//         let ret = self.inner.mkdir(name)?;
//         self.meta.lock_inner_with(|inner| {
//             inner.data_len = self.inner.disk_space();
//             inner.modify_time = TimeSpec::from(time::curr_time());
//         });
//         Ok(ret)
//     }

//     pub fn mknod(&self, name: &str, mode: InodeMode) -> KResult<Arc<DynPagedInode>> {
//         let ret = self.inner.mknod(name, mode)?;
//         self.meta.lock_inner_with(|inner| {
//             inner.data_len = self.inner.disk_space();
//             inner.modify_time = TimeSpec::from(time::curr_time());
//         });
//         Ok(ret)
//     }

//     pub fn unlink(&self, name: &str) -> KResult<()> {
//         self.inner.unlink(name)?;
//         self.meta.lock_inner_with(|inner| {
//             inner.data_len = self.inner.disk_space();
//             inner.modify_time = TimeSpec::from(time::curr_time());
//         });
//         Ok(())
//     }

//     pub fn read_dir(&self, dentry: &Arc<DEntryDir>) -> KResult<()> {
//         // TODO: [low] `read_dir` 可以记录一个状态表示是否已经全部读入，有的话就不用调用底层文件系统
//         self.inner.read_dir(dentry)?;
//         self.meta
//             .lock_inner_with(|inner| inner.access_time = TimeSpec::from(time::curr_time()));
//         Ok(())
//     }
// }

pub trait BytesInodeBackend: Send + Sync + 'static {
    fn meta(&self) -> &InodeMeta;
    fn read_at(&self, buf: &mut [u8], offset: u64) -> KResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> KResult<usize>;

    /// `self` 必须来自于一个 `Arc<Self>`
    unsafe fn as_paged_inode(self: *const Self) -> Option<Arc<PagedInode<dyn BytesInodeBackend>>> {
        None
    }
}

/// 可以按页级别进行读写的 inode，一般应该是块设备做后备
pub struct PagedInode<T: ?Sized> {
    page_cache: RwLock<PageCache>,
    backend: T,
}

impl<T> PagedInode<T> {
    pub fn new(backend: T) -> Self {
        Self {
            page_cache: RwLock::new(PageCache::new()),
            backend,
        }
    }
}

impl<T: BytesInodeBackend> BytesInodeBackend for PagedInode<T> {
    fn meta(&self) -> &InodeMeta {
        self.backend.meta()
    }

    fn read_at(&self, buf: &mut [u8], offset: u64) -> KResult<usize> {
        let meta = self.meta();
        let data_len = meta.lock_inner_with(|inner| inner.data_len);

        if offset >= data_len {
            return Ok(0);
        }

        let read_end = usize::min(buf.len(), (data_len - offset) as usize);
        let mut nread = 0;

        while nread < read_end {
            let page_id = (offset + nread as u64) >> PAGE_SIZE_BITS;
            let page_offset = ((offset + nread as u64) & PAGE_OFFSET_MASK as u64) as usize;
            let page = self.get_or_init_page(page_id);

            // 检查页状态，如有必要则读后备文件
            if page.state.load(Ordering::SeqCst) == PageState::Invalid {
                let mut _guard = block_on(page.state_guard.lock());
                if page.state.load(Ordering::SeqCst) == PageState::Invalid {
                    self.backend.read_at(
                        page.inner.frame_mut().as_page_bytes_mut(),
                        page_id << PAGE_SIZE_BITS,
                    )?;
                    page.state.store(PageState::Synced, Ordering::SeqCst);
                }
            }
            let frame = page.inner.frame();

            let copy_len = usize::min(read_end - nread, PAGE_SIZE - page_offset);
            buf[nread..nread + copy_len]
                .copy_from_slice(&frame.as_page_bytes()[page_offset..page_offset + copy_len]);
            nread += copy_len;
        }
        meta.lock_inner_with(|inner| inner.access_time = TimeSpec::from(time::curr_time()));

        Ok(nread)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> KResult<usize> {
        let meta = self.meta();
        let curr_data_len = meta.lock_inner_with(|inner| inner.data_len);
        let curr_last_page_id = curr_data_len >> PAGE_SIZE_BITS;

        // 写范围是 offset..offset + buf.len()。
        // 中间可能有一些页被完全覆盖，因此可以直接设为 Dirty 而不需要读
        let full_page_range = (offset & (!PAGE_OFFSET_MASK) as u64)
            ..(offset + buf.len() as u64).next_multiple_of(PAGE_SIZE as u64);

        let mut nwrite = 0;

        while nwrite < buf.len() {
            let page_id = (offset + nwrite as u64) >> PAGE_SIZE_BITS as u64;
            let page_offset = ((offset + nwrite as u64) & PAGE_OFFSET_MASK as u64) as usize;
            let page = self.get_or_init_page(page_id);

            let mut frame;
            if page.state.load(Ordering::SeqCst) == PageState::Invalid {
                let mut _guard = block_on(page.state_guard.lock());
                frame = page.inner.frame_mut();
                if page_id <= curr_last_page_id
                    && full_page_range.contains(&page_id)
                    && page.state.load(Ordering::SeqCst) == PageState::Invalid
                {
                    self.backend
                        .read_at(frame.as_page_bytes_mut(), page_id << PAGE_SIZE_BITS)?;
                }
                page.state.store(PageState::Dirty, Ordering::SeqCst);
            } else {
                frame = page.inner.frame_mut();
            }

            let copy_len = usize::min(buf.len() - nwrite, PAGE_SIZE - page_offset);
            frame.as_page_bytes_mut()[page_offset..page_offset + copy_len]
                .copy_from_slice(&buf[nwrite..nwrite + copy_len]);
            nwrite += copy_len;
        }
        meta.lock_inner_with(|inner| {
            inner.access_time = TimeSpec::from(time::curr_time());
            inner.modify_time = inner.access_time;
            if inner.data_len < offset + buf.len() as u64 {
                inner.data_len = offset + buf.len() as u64;
                inner.change_time = inner.access_time;
            }
        });

        Ok(nwrite)
    }

    /// `self` 必须来自于一个 `Arc<Self>`
    unsafe fn as_paged_inode(self: *const Self) -> Option<Arc<PagedInode<dyn BytesInodeBackend>>> {
        let this = unsafe { Arc::from_raw(self) };
        Some(this.unsize(unsafe { Coercion::new(|p| p as _) }))
    }
}

impl<T: ?Sized> PagedInode<T> {
    pub fn lock_page_cache(&self) -> RwLockReadGuard<'_, PageCache> {
        self.page_cache.read()
    }
}

impl<T: ?Sized> Deref for PagedInode<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

impl<T: ?Sized + BytesInodeBackend> PagedInode<T> {
    fn get_or_init_page(&self, page_id: u64) -> Arc<BackedPage> {
        let page = self.page_cache.read().get(page_id);
        page.unwrap_or_else(|| self.page_cache.write().create(page_id))
    }
}

pub enum DynInode {
    Dir(Arc<DynDirInode>),
    Bytes(Arc<DynBytesInode>),
}

pub macro DynDirInodeCoercion() {
    #[allow(unused_unsafe)]
    unsafe {
        ::unsize::Coercion::new({
            #[allow(unused_parens)]
            fn coerce<'lt>(
                p: *const (impl DirInodeBackend + 'lt),
            ) -> *const (dyn DirInodeBackend + 'lt) {
                p
            }
            coerce
        })
    }
}

pub macro DynBytesInodeCoercion() {
    #[allow(unused_unsafe)]
    unsafe {
        ::unsize::Coercion::new({
            #[allow(unused_parens)]
            fn coerce<'lt>(
                p: *const (impl BytesInodeBackend + 'lt),
            ) -> *const (dyn BytesInodeBackend + 'lt) {
                p
            }
            coerce
        })
    }
}
