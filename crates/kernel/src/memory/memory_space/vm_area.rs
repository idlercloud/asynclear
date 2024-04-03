use core::assert_matches::assert_matches;
use core::ops::Range;

use alloc::collections::BTreeMap;
use common::config::{PAGE_SIZE, PA_TO_VA};
use triomphe::Arc;

use crate::memory::{
    frame_allocator::Frame, kernel_pa_to_va, kernel_ppn_to_vpn, MapPermission, PTEFlags, PageTable,
    PhysAddr, PhysPageNum, VirtAddr, VirtPageNum,
};

#[derive(Debug, Clone)]
pub struct VmArea {
    pub vpn_range: Range<VirtPageNum>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl VmArea {
    pub fn new_framed(start_va: VirtAddr, end_va: VirtAddr, map_perm: MapPermission) -> Self {
        let start_vpn: VirtPageNum = start_va.vpn_floor();
        let end_vpn: VirtPageNum = end_va.vpn_ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            map_type: MapType::Framed(FramedMap {
                map: BTreeMap::new(),
            }),
            map_perm,
        }
    }

    /// 内核中采取的线性映射
    pub fn kernel_map(start_pa: PhysAddr, end_pa: PhysAddr, map_perm: MapPermission) -> Self {
        let start_vpn = kernel_pa_to_va(start_pa).vpn_floor();
        let end_vpn = kernel_pa_to_va(end_pa).vpn_ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            map_type: MapType::Linear { offset: PA_TO_VA },
            map_perm,
        }
    }

    pub fn from_another(another: &VmArea) -> Self {
        Self {
            vpn_range: another.vpn_range.clone(),
            map_type: another.map_type.clone(),
            map_perm: another.map_perm,
        }
    }

    pub fn len(&self) -> usize {
        self.vpn_range.end.0.saturating_sub(self.vpn_range.start.0) * PAGE_SIZE
    }

    // pub fn is_empty(&self) -> bool {
    //     self.len() == 0
    // }

    fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn;
        match &mut self.map_type {
            MapType::Linear { offset } => {
                ppn = PhysPageNum(vpn.0 - *offset / PAGE_SIZE);
            }
            MapType::Framed(framed_map) => {
                let frame = Frame::alloc().unwrap();
                ppn = frame.ppn();
                framed_map.map.insert(vpn, Arc::new(frame));
            }
        }
        page_table.map(vpn, ppn, PTEFlags::from(self.map_perm));
    }

    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if let MapType::Framed(framed_map) = &mut self.map_type {
            framed_map.map.remove(&vpn);
        }
        page_table.unmap(vpn);
    }

    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.clone() {
            self.map_one(page_table, vpn);
        }
    }

    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.clone() {
            self.unmap_one(page_table, vpn);
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

    /// 约定：当前逻辑段必须是 `Framed` 的。而且 `data` 的长度不得超过逻辑段长度。
    pub fn copy_data(&mut self, page_table: &mut PageTable, start_offset: usize, data: &[u8]) {
        // TODO: [mid] 调整 API 使其只能对 Framed 使用，避免运行时检查
        assert_matches!(self.map_type, MapType::Framed { .. });
        debug_assert!(start_offset < PAGE_SIZE);
        debug_assert!(data.len() <= self.len());
        let mut curr_vpn = self.vpn_range.start;

        let (first_block, rest) = data.split_at((PAGE_SIZE - start_offset).min(data.len()));

        unsafe {
            kernel_ppn_to_vpn(page_table.translate(curr_vpn).unwrap())
                .copy_from(start_offset, first_block);

            curr_vpn.0 += 1;

            for chunk in rest.chunks(PAGE_SIZE) {
                let dst = page_table.translate(curr_vpn).unwrap();
                kernel_ppn_to_vpn(dst).copy_from(0, chunk);
                curr_vpn.0 += 1;
            }
        }
    }
}

/// 描述逻辑段内所有虚拟页映射到物理页的方式
#[derive(Clone, Debug)]
enum MapType {
    /// 线性映射，即物理地址到虚地址有一个固定的 offset。
    /// 内核中这个量是 `PA_TO_VA` 即 `0xFFFF_FFFF_0000_0000`
    Linear { offset: usize },
    /// 需要分配物理页帧
    Framed(FramedMap),
}

#[derive(Clone, Debug)]
struct FramedMap {
    /// 这些保存的物理页帧用于存放实际的内存数据
    ///
    /// 而 `PageTable` 所拥有的的物理页仅用于存放页表节点数据，因此不会冲突
    map: BTreeMap<VirtPageNum, Arc<Frame>>,
}
