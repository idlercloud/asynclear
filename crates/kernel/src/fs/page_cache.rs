//! Page Cache 加锁全过程：
//!
//! 1. 上层在页缓存中查询给定页是否存在
//! 2. 如果不存在，创建一个新的页，其状态设置为无效
//! 3. 检查该页的状态
//!     - 如果是无效的，获取写锁，并发起磁盘读取，直到为有效
//!         - 如果是写操作，则维持写锁
//!         - 如果是读操作，则降级为读锁
//!     - 如果是有效的，根据目的获取写锁或者读锁
//! 4. 操作完毕后释放锁

use alloc::collections::BTreeMap;

use async_lock::Mutex as SleepMutex;
use atomic::Atomic;
use triomphe::Arc;

use crate::memory::{Frame, Page};

pub struct PageCache {
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
}

pub struct BackedPage {
    pub(super) inner: Page,
    pub(super) state_guard: SleepMutex<()>,
    pub(super) state: Atomic<PageState>,
}

#[derive(bytemuck::NoUninit, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PageState {
    Invalid,
    Synced,
    Dirty,
}
