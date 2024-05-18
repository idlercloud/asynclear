use alloc::collections::BTreeMap;

use async_lock::Mutex as SleepMutex;
use atomic::Atomic;
use triomphe::Arc;

use crate::memory::{Frame, Page};

pub struct PageCache {
    // TODO: 也许页缓存可以用 `HashMap`，代价可能是减缓初次 `mmap`
    /// 文件页号 -> 页
    pages: BTreeMap<usize, Arc<BackedPage>>,
}

impl PageCache {
    pub fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }

    pub fn get(&self, page_id: usize) -> Option<Arc<BackedPage>> {
        self.pages.get(&page_id).cloned()
    }

    pub fn create(&mut self, page_id: usize) -> Arc<BackedPage> {
        let new_page: Arc<BackedPage> = Arc::new(BackedPage {
            inner: Page::with_frame(Frame::alloc().unwrap()),
            state_guard: SleepMutex::new(()),
            state: Atomic::new(PageState::Invalid),
        });
        let maybe_old = self.pages.insert(page_id, Arc::clone(&new_page));
        assert!(maybe_old.is_none());
        new_page
    }

    pub fn pages(&self) -> &BTreeMap<usize, Arc<BackedPage>> {
        &self.pages
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
