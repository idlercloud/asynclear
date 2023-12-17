//! Implementation of [`MapArea`] and [`MemorySet`].

use super::kernel_va_to_pa;
use super::{
    frame_alloc, kernel_ppn_to_vpn, FrameTracker, PTEFlags, PageTable, PhysAddr, PhysPageNum,
    VirtAddr, VirtPageNum,
};
use alloc::{collections::BTreeMap, sync::Arc};
use bitflags::bitflags;
use core::{assert_matches::assert_matches, ops::Range};
use defines::{
    config::{LOW_ADDRESS_END, MEMORY_END, MMAP_START, MMIO, PAGE_SIZE, PAGE_SIZE_BITS, PA_TO_VA},
    error::{errno, Result},
};
use goblin::elf::Elf;
use goblin::elf64::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use klocks::Lazy;
use riscv::register::satp;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    // fn edata();
    // fn sbss();
    fn ebss();
    fn ekernel();
}

pub static KERNEL_SPACE: Lazy<MemorySet> = Lazy::new(MemorySet::new_kernel);

/// Get the token of the kernel memory space
pub fn kernel_token() -> usize {
    KERNEL_SPACE.token()
}

bitflags! {
    /// 对应于 PTE 中权限位的映射权限：`R W X U`
    #[derive(Clone, Copy, Debug)]
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
    }
}

/// memory set structure, controls virtual-memory space
pub struct MemorySet {
    page_table: PageTable,
    // 起始 vpn 映射到 MapArea
    areas: BTreeMap<VirtPageNum, MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::with_root(),
            areas: BTreeMap::new(),
        }
    }

    fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();

        memory_set.push(
            MapArea::kernel_map(
                kernel_va_to_pa(VirtAddr(stext as usize)),
                kernel_va_to_pa(VirtAddr(etext as usize)),
                MapPermission::R | MapPermission::X | MapPermission::G,
            ),
            0,
            None,
        );

        // 注：旧版的 Linux 中，text 段和 rodata 段是合并在一起的，这样可以减少一次映射
        // 新版本则独立开来了，参考 https://stackoverflow.com/questions/44938745/rodata-section-loaded-in-executable-page
        memory_set.push(
            MapArea::kernel_map(
                kernel_va_to_pa(VirtAddr(srodata as usize)),
                kernel_va_to_pa(VirtAddr(erodata as usize)),
                MapPermission::R | MapPermission::G,
            ),
            0,
            None,
        );

        // .data 段和 .bss 段的访问限制相同，所以可以放到一起
        memory_set.push(
            MapArea::kernel_map(
                kernel_va_to_pa(VirtAddr(sdata as usize)),
                kernel_va_to_pa(VirtAddr(ebss as usize)),
                MapPermission::R | MapPermission::W | MapPermission::G,
            ),
            0,
            None,
        );

        // TODO: 这里也许可以 Huge Page 映射？
        memory_set.push(
            MapArea::kernel_map(
                kernel_va_to_pa(VirtAddr(ekernel as usize)),
                PhysAddr(MEMORY_END),
                MapPermission::R | MapPermission::W | MapPermission::G,
            ),
            0,
            None,
        );

        // MMIO 映射
        for &(start, len) in MMIO {
            memory_set.push(
                MapArea::kernel_map(
                    PhysAddr(start),
                    PhysAddr(start + len),
                    MapPermission::R | MapPermission::W | MapPermission::G,
                ),
                0,
                None,
            );
        }
        memory_set
    }

    pub fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    /// 加载所有段，返回 ELF 数据的结束地址
    ///
    /// 记得调用前清理地址空间，否则可能 panic
    pub fn load_sections(&mut self, elf: &Elf<'_>, elf_data: &[u8]) -> VirtAddr {
        let mut elf_end = VirtAddr(0);
        for ph in &elf.program_headers {
            if ph.p_type == PT_LOAD {
                // Program header 在 ELF 中的偏移为 0，所以其地址就是 ELF 段的起始地址
                let start_va = VirtAddr(ph.p_vaddr as usize);
                let start_offset = start_va.page_offset();
                let end_va = VirtAddr((ph.p_vaddr + ph.p_memsz) as usize);
                elf_end = VirtAddr::max(elf_end, end_va);
                let mut map_perm = MapPermission::U;
                if ph.p_flags & PF_R != 0 {
                    map_perm |= MapPermission::R;
                }
                if ph.p_flags & PF_W != 0 {
                    map_perm |= MapPermission::W;
                }
                if ph.p_flags & PF_X != 0 {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(
                    start_va,
                    end_va,
                    MapType::Framed {
                        data_frames: BTreeMap::new(),
                    },
                    map_perm,
                );
                self.push(map_area, start_offset, Some(&elf_data[ph.file_range()]));
            }
        }
        elf_end
    }

    /// 映射高地址中的内核段，注意不持有它们的所有权
    pub fn map_kernel_areas(&mut self, kernel_pt: &PageTable) {
        // 用户地址空间中，高地址是内核的部分
        // 具体而言，就是 [0xffff_ffff_8000_000, 0xffff_ffff_ffff_fff]
        // 以及 [0xffff_ffff_0000_0000, 0xffff_ffff_3fff_ffff]（MMIO 所在的大页）
        // 也就是内核根页表的第 508、510、511 项
        unsafe {
            // 这些需要映射到用户的页表中
            for line in [508, 510, 511] {
                let user_pte = self.page_table.root_pte_mut(line);
                let kernel_pte = kernel_pt.root_pte(line);
                user_pte.bits = kernel_pte.bits;
            }
        }
    }

    /// 从另一地址空间复制
    pub fn from_existed_user(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();
        for (_, area) in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, 0, None);
            // 从该地址空间复制数据
            for vpn in area.vpn_range.clone() {
                let src_ppn = user_space.translate(vpn).unwrap();
                let dst_ppn = memory_set.translate(vpn).unwrap();
                // let physical_memory_begin_frame: usize =
                //     kernel_va_to_pa(VirtAddr(ekernel as usize)).ceil().0;
                // info!(
                //     "Copy from {} to {}",
                //     src_ppn.0 - physical_memory_begin_frame,
                //     dst_ppn.0 - physical_memory_begin_frame
                // );
                unsafe {
                    kernel_ppn_to_vpn(dst_ppn)
                        .as_page_bytes_mut()
                        .copy_from_slice(kernel_ppn_to_vpn(src_ppn).as_page_bytes());
                }
            }
        }
        // 已存在的地址空间高地址部分也已经映射了，所以从它那里复制内核空间也是可以的
        memory_set.map_kernel_areas(&user_space.page_table);
        memory_set
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    /// 需保证 `heap_start` < `new_vpn`，且还有足够的虚地址和物理空间可以映射
    pub fn set_user_brk(&mut self, new_end: VirtPageNum, heap_start: VirtPageNum) {
        // 堆区已经映射过了，就扩张或者收缩。否则插入堆区
        if let Some(map_area) = self.areas.get_mut(&heap_start) {
            let curr_vpn = map_area.end();
            if curr_vpn >= new_end {
                map_area.shrink(new_end, &mut self.page_table);
            } else {
                map_area.expand(new_end, &mut self.page_table);
            }
        } else {
            self.insert_framed_area(
                heap_start,
                new_end,
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
        }
    }

    /// 尝试根据 `vm_range` 进行映射
    pub fn try_map(
        &mut self,
        vpn_range: Range<VirtPageNum>,
        perm: MapPermission,
        fixed: bool,
    ) -> Result<isize> {
        if fixed {
            // TODO: 应当 unmap 与其相交的部分。不过，如果是一些不该 unmap 的区域，是否该返回错误？
            self.insert_framed_area(vpn_range.start, vpn_range.end, perm);
            Ok(vpn_range.start.0 as isize)
        } else {
            // 尝试找到一个合适的段来映射
            let mut start = VirtPageNum(MMAP_START >> PAGE_SIZE_BITS);
            let len = vpn_range.end.0 - vpn_range.start.0;
            for area in self.areas.values() {
                // 要控制住不溢出低地址空间的上限
                if area.vpn_range.start > start
                    && start + len <= VirtPageNum(LOW_ADDRESS_END >> PAGE_SIZE_BITS)
                {
                    // 找到可映射的段
                    if start + len <= area.vpn_range.start {
                        // TODO: 匿名映射的话，按照约定应当全部初始化为 0
                        self.insert_framed_area(start, start + len, perm);
                        return Ok(start.page_start().0 as isize);
                    }
                    start = area.vpn_range.end;
                }
            }
            Err(errno::ENOMEM)
        }
    }

    /// 插入帧映射的一个内存段，假定是不会造成冲突的
    pub fn insert_framed_area(
        &mut self,
        start_vpn: VirtPageNum,
        end_vpn: VirtPageNum,
        perm: MapPermission,
    ) {
        self.push(
            MapArea {
                vpn_range: start_vpn..end_vpn,
                map_type: MapType::new_framed(),
                map_perm: perm,
            },
            0,
            None,
        );
    }

    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some(mut area) = self.areas.remove(&start_vpn) {
            area.unmap(&mut self.page_table);
        }
    }

    /// `start_offset` 是数据在页中开始的偏移
    pub fn push(&mut self, mut map_area: MapArea, start_offset: usize, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, start_offset, data);
        }
        self.areas.insert(map_area.vpn_range.start, map_area);
    }

    /// 如有必要就切换页表，只在内核态调用，执行流不会跳变
    pub fn activate(&self) {
        let old_root = satp::read().bits();
        let new_root = self.page_table.token();
        if new_root != old_root {
            satp::write(new_root);
            self.flush_tlb(None);
        }
    }

    /// 刷新 tlb，可选刷新一部分，或者全部刷新
    pub fn flush_tlb(&self, vaddr: Option<VirtAddr>) {
        if let Some(vaddr) = vaddr {
            unsafe {
                riscv::asm::sfence_vma(0, vaddr.0);
            }
        } else {
            unsafe {
                riscv::asm::sfence_vma_all();
            }
        }
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PhysPageNum> {
        self.page_table.translate(vpn)
    }

    pub fn recycle_all_pages(&mut self) {
        self.areas.clear();
    }

    /// 只回收进程低 256GiB 部分的页面，也就是用户进程专属的页（包括页表）
    pub fn recycle_user_pages(&mut self) {
        // TODO: 等等，Memory.areas 中是不是其实只存放了用户地址的映射？
        // 也就是只保留高地址的空间
        self.areas
            .retain(|vpn, _| vpn.0 >= LOW_ADDRESS_END / PAGE_SIZE);
        self.page_table.clear_except_root();
        // 根页表要处理下，把用户地址的页表项去除，以防已经回收的页仍然能被访问
        unsafe {
            self.page_table.root_page()[0..PAGE_SIZE / 2].fill(0);
        }
        // FIXME: 为了安全性，也许应该刷新 TLB？
        self.flush_tlb(None);
        // 回收了用户页表的进程不应该去访问用户数据了
        // 一般而言都是成为了僵尸进程，那么其实可能确实不会访问
        // 所以应该不太会去访问。不过也许会有误操作？
        // 但是，除去系统调用之外，会有其他访问用户数据的操作吗？
        // 另外，也许可以通过 SUM 标志位来控制？
    }
}

/// 描述逻辑段内所有虚拟页映射到物理页的方式
#[derive(Debug, Clone)]
pub enum MapType {
    /// 线性映射，即物理地址到虚地址有一个固定的 offset。
    /// 内核中这个量是 PA_TO_VA 即 0xFFFF_FFFF_0000_0000
    Linear { offset: usize },
    /// 需要分配物理页帧
    Framed {
        /// 这些保存的物理页帧用于存放实际的内存数据
        ///
        /// 而 PageTable 所拥有的的物理页仅用于存放页表节点数据，因此不会冲突
        data_frames: BTreeMap<VirtPageNum, Arc<FrameTracker>>,
    },
}

impl MapType {
    /// 空的帧映射
    pub fn new_framed() -> Self {
        Self::Framed {
            data_frames: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapArea {
    pub vpn_range: Range<VirtPageNum>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.vpn_floor();
        let end_vpn: VirtPageNum = end_va.vpn_ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            map_type,
            map_perm,
        }
    }
    /// 内核中采取的线性映射
    pub fn kernel_map(start_pa: PhysAddr, end_pa: PhysAddr, map_perm: MapPermission) -> Self {
        let start_vpn = VirtAddr(start_pa.0 + PA_TO_VA).vpn_floor();
        let end_vpn = VirtAddr(end_pa.0 + PA_TO_VA).vpn_ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            map_type: MapType::Linear { offset: PA_TO_VA },
            map_perm,
        }
    }
    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: another.vpn_range.clone(),
            map_type: another.map_type.clone(),
            map_perm: another.map_perm,
        }
    }
    pub fn len(&self) -> usize {
        self.vpn_range.end.0.saturating_sub(self.vpn_range.start.0) * PAGE_SIZE
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn;
        match &mut self.map_type {
            MapType::Linear { offset } => {
                ppn = PhysPageNum(vpn.0 - *offset / PAGE_SIZE);
            }
            MapType::Framed { data_frames } => {
                let frame = frame_alloc(1).unwrap();
                ppn = frame.ppn;
                data_frames.insert(vpn, Arc::new(frame));
            }
        }
        page_table.map(vpn, ppn, PTEFlags::from_bits_truncate(self.map_perm.bits()));
    }

    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if let MapType::Framed { data_frames } = &mut self.map_type {
            data_frames.remove(&vpn);
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
    #[inline]
    pub fn end(&self) -> VirtPageNum {
        self.vpn_range.end
    }
    /// 尝试收缩末尾区域
    pub fn shrink(&mut self, new_end: VirtPageNum, page_table: &mut PageTable) {
        for vpn in new_end..self.end() {
            self.unmap_one(page_table, vpn);
        }
        self.vpn_range.end = new_end;
    }
    /// 尝试扩展末尾区域
    pub fn expand(&mut self, new_end: VirtPageNum, page_table: &mut PageTable) {
        for vpn in self.end()..new_end {
            self.map_one(page_table, vpn);
        }
        self.vpn_range.end = new_end;
    }
    /// 约定：当前逻辑段必须是 `Framed` 的。而且 `data` 的长度不得超过逻辑段长度。
    pub fn copy_data(&mut self, page_table: &mut PageTable, start_offset: usize, data: &[u8]) {
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
