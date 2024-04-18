use alloc::{collections::BTreeMap, sync::Arc};

use crate::memory::Page;

pub struct PageCache {
    /// 文件页号 -> 页
    pages: BTreeMap<usize, Arc<Page>>,
}
