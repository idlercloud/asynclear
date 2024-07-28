use alloc::collections::BTreeMap;
use core::ops::{Bound, RangeBounds};

use async_lock::Mutex as SleepMutex;
use atomic::Atomic;
use klocks::{RwLock, RwLockReadGuard};
use triomphe::Arc;

use crate::memory::{Frame, Page};

pub struct PageCache {
    // TODO: 也许页缓存可以用 `HashMap`，代价可能是减缓初次 `mmap`
    /// 文件页号 -> 页
    pages: RwLock<BTreeMap<u64, Arc<BackedPage>>>,
}

impl PageCache {
    pub fn new() -> Self {
        Self {
            pages: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn get(&self, page_id: u64) -> Option<Arc<BackedPage>> {
        self.pages.read().get(&page_id).cloned()
    }

    pub fn create(&self, page_id: u64) -> Arc<BackedPage> {
        let new_page: Arc<BackedPage> = Arc::new(BackedPage {
            inner: Page::with_frame(Frame::alloc().unwrap()),
            state_guard: SleepMutex::new(()),
            state: Atomic::new(PageState::Invalid),
        });
        let maybe_old = self.pages.write().insert(page_id, Arc::clone(&new_page));
        assert!(maybe_old.is_none());
        new_page
    }

    pub fn get_or_init_page(&self, page_id: u64) -> Arc<BackedPage> {
        self.get(page_id).unwrap_or_else(|| self.create(page_id))
    }

    pub fn lock_pages(&self) -> RwLockReadGuard<'_, BTreeMap<u64, Arc<BackedPage>>> {
        self.pages.read()
    }

    pub fn free_pages(&self, range: impl RangeBounds<u64>) {
        let mut pages = self.pages.write();
        let (start, end) = (range.start_bound(), range.end_bound());
        let mut freed_part = match start {
            Bound::Unbounded => core::mem::take(&mut *pages),
            Bound::Included(left) => pages.split_off(&left),
            _ => unreachable!("start bound should not be excluded"),
        };
        let mut right_part = match end {
            Bound::Unbounded => BTreeMap::new(),
            Bound::Excluded(right) => freed_part.split_off(&right),
            Bound::Included(right) => freed_part.split_off(&(right + 1)),
        };
        pages.append(&mut right_part);
    }
}

pub struct BackedPage {
    pub(super) inner: Page,
    pub(super) state_guard: SleepMutex<()>,
    pub(super) state: Atomic<PageState>,
}

impl BackedPage {
    pub fn inner_page(&self) -> &Page {
        &self.inner
    }
}

#[derive(bytemuck::NoUninit, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PageState {
    Invalid,
    Synced,
    Dirty,
}
