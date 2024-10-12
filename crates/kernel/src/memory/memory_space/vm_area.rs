use alloc::collections::{BTreeMap, BTreeSet};
use core::ops::{Deref, Range};

use common::config::PAGE_SIZE;
use triomphe::Arc;

use crate::{
    fs::{DynBytesInode, InodeMode},
    memory::{frame_allocator::Frame, kernel_ppn_to_vpn, page::Page, MapPermission, PTEFlags, PageTable, VirtPageNum},
};

/// 采取帧式映射的一块（用户）虚拟内存区域
pub struct FramedVmArea {
    vpn_range: Range<VirtPageNum>,
    perm: MapPermission,
    area_type: AreaType,
    // 暂时而言，整个 area 要么都是有文件后备，要么都是无文件后备
    // 但是实现 private mmap 的话可能就不是了
    unbacked_map: BTreeMap<VirtPageNum, Arc<Page>>,
    backed_inode: Option<BackedInode>,
    backed_pages: BTreeSet<VirtPageNum>,
    backed_inode_page_id: u64,
}

#[derive(Clone)]
pub struct BackedInode(Arc<DynBytesInode>);

impl BackedInode {
    pub fn new(inode: &Arc<DynBytesInode>) -> Option<Self> {
        if inode.meta().mode() == InodeMode::Regular {
            Some(Self(Arc::clone(inode)))
        } else {
            None
        }
    }
}

impl Deref for BackedInode {
    type Target = Arc<DynBytesInode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    Lazy,
    Mmap,
}

impl FramedVmArea {
    pub(super) fn new(vpn_range: Range<VirtPageNum>, perm: MapPermission, area_type: AreaType) -> Self {
        Self {
            vpn_range,
            unbacked_map: BTreeMap::new(),
            perm,
            area_type,
            backed_inode: None,
            backed_pages: BTreeSet::new(),
            backed_inode_page_id: 0,
        }
    }

    pub fn vpn_range(&self) -> Range<VirtPageNum> {
        self.vpn_range.clone()
    }

    pub fn perm(&self) -> MapPermission {
        self.perm
    }

    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    pub fn unbacked_map(&self) -> &BTreeMap<VirtPageNum, Arc<Page>> {
        &self.unbacked_map
    }

    pub fn backed_inode(&self) -> Option<&BackedInode> {
        self.backed_inode.as_ref()
    }

    pub fn backed_inode_page_id(&self) -> u64 {
        self.backed_inode_page_id
    }

    pub fn len(&self) -> usize {
        self.vpn_range.end.0.saturating_sub(self.vpn_range.start.0) * PAGE_SIZE
    }

    pub fn init_backed_inode(&mut self, inode: BackedInode, inode_page_id: u64, page_table: &mut PageTable) {
        // 先把已经在页缓存中的映射好
        {
            let n_pages = self.vpn_range.end.0 - self.vpn_range.start.0;
            let page_cache = inode.meta().page_cache().lock_pages();
            for (&page_id, page) in page_cache.range(inode_page_id..inode_page_id + n_pages as u64) {
                let frame = page.inner_page().frame();
                let vpn = self.vpn_range.start + (page_id - inode_page_id) as usize;
                page_table.map(vpn, frame.ppn(), PTEFlags::from(self.perm));
                self.backed_pages.insert(vpn);
            }
        }
        self.backed_inode = Some(inode);
        self.backed_inode_page_id = inode_page_id;
    }

    // 只能给
    pub fn ensure_allocated(&mut self, vpn: VirtPageNum, page_table: &mut PageTable) -> &Arc<Page> {
        assert!(self.area_type == AreaType::Lazy);
        let entry = self.unbacked_map.entry(vpn);
        entry.or_insert_with(|| {
            let frame = Frame::alloc().unwrap();
            let ppn = frame.ppn();
            page_table.map(vpn, ppn, PTEFlags::from(self.perm));
            Arc::new(Page::with_frame(frame))
        })
    }

    pub(super) unsafe fn map_with_data(&mut self, page_table: &mut PageTable, data: &[u8], mut page_offset: usize) {
        debug_assert!(data.len() + page_offset <= self.len());
        let mut start = 0;
        for vpn in self.vpn_range() {
            let frame = Frame::alloc().unwrap();
            let ppn = frame.ppn();
            self.unbacked_map.insert(vpn, Arc::new(Page::with_frame(frame)));
            page_table.map(vpn, ppn, PTEFlags::from(self.perm));
            let len = usize::min(data.len() - start, PAGE_SIZE - page_offset);
            unsafe {
                kernel_ppn_to_vpn(ppn).as_page_bytes_mut()[page_offset..page_offset + len]
                    .copy_from_slice(&data[start..start + len]);
            }
            page_offset = 0;
            start += len;
        }
    }

    pub(super) fn unmap(&mut self, page_table: &mut PageTable) {
        for &mapped in self.unbacked_map.keys().chain(&self.backed_pages) {
            page_table.unmap(mapped);
        }
        self.unbacked_map.clear();
        self.backed_inode = None;
        self.backed_pages.clear();
        self.backed_inode_page_id = 0;
    }

    /// 尝试收缩末尾区域
    pub fn shrink(&mut self, new_end: VirtPageNum, page_table: &mut PageTable) {
        // TODO: vm area 收缩暂时不考虑文件后备
        assert!(self.area_type == AreaType::Lazy);
        {
            let split = self.unbacked_map.split_off(&new_end);
            for &mapped in split.keys() {
                page_table.unmap(mapped);
            }
        }
        self.vpn_range.end = new_end;
    }

    /// 尝试扩展末尾区域
    pub fn expand(&mut self, new_end: VirtPageNum) {
        self.vpn_range.end = new_end;
    }
}
