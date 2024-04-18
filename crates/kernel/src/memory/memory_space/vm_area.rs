use core::ops::Range;

use alloc::collections::BTreeMap;
use common::config::PAGE_SIZE;
use klocks::SpinMutexGuard;
use triomphe::Arc;

use crate::memory::{
    frame_allocator::Frame, kernel_ppn_to_vpn, page::Page, MapPermission, PTEFlags, PageTable,
    VirtAddr, VirtPageNum,
};

/// 采取帧式映射的一块（用户）虚拟内存区域
#[derive(Clone)]
pub struct FramedVmArea {
    vpn_range: Range<VirtPageNum>,
    map: BTreeMap<VirtPageNum, Arc<Page>>,
    perm: MapPermission,
    area_type: AreaType,
}

#[derive(Clone, Copy, Debug)]
pub enum AreaType {
    Stack,
    Elf,
}

impl FramedVmArea {
    pub(super) fn new(va_range: Range<VirtAddr>, perm: MapPermission, area_type: AreaType) -> Self {
        let start_vpn = va_range.start.vpn_floor();
        let end_vpn = va_range.end.vpn_ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            map: BTreeMap::new(),
            perm,
            area_type,
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

    pub fn mapped_frame(&self, vpn: VirtPageNum) -> Option<SpinMutexGuard<'_, Frame>> {
        self.map.get(&vpn).map(|page| page.frame())
    }

    pub fn len(&self) -> usize {
        self.vpn_range.end.0.saturating_sub(self.vpn_range.start.0) * PAGE_SIZE
    }

    pub fn ensure_allocated(&mut self, vpn: VirtPageNum, page_table: &mut PageTable) -> &Arc<Page> {
        let entry = self.map.entry(vpn);
        entry.or_insert_with(|| {
            let frame = Frame::alloc().unwrap();
            let ppn = frame.ppn();
            page_table.map(vpn, ppn, PTEFlags::from(self.perm));
            Arc::new(Page::new(frame))
        })
    }

    pub(super) unsafe fn map_with_data(
        &mut self,
        page_table: &mut PageTable,
        data: &[u8],
        mut page_offset: usize,
    ) {
        debug_assert!(data.len() + page_offset <= self.len());
        let mut start = 0;
        for vpn in self.vpn_range() {
            let frame = Frame::alloc().unwrap();
            let ppn = frame.ppn();
            self.map.insert(vpn, Arc::new(Page::new(frame)));
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
        for vpn in self.vpn_range() {
            if self.map.remove(&vpn).is_some() {
                page_table.unmap(vpn);
            }
        }
    }

    // #[inline]
    // pub fn end(&self) -> VirtPageNum {
    //     self.vpn_range.end
    // }

    // /// 尝试收缩末尾区域
    // pub fn shrink(&mut self, new_end: VirtPageNum, page_table: &mut PageTable) {
    //     for vpn in new_end..self.end() {
    //         self.unmap_one(page_table, vpn);
    //     }
    //     self.vpn_range.end = new_end;
    // }

    // /// 尝试扩展末尾区域
    // pub fn expand(&mut self, new_end: VirtPageNum, page_table: &mut PageTable) {
    //     for vpn in self.end()..new_end {
    //         self.map_one(page_table, vpn);
    //     }
    //     self.vpn_range.end = new_end;
    // }
}
