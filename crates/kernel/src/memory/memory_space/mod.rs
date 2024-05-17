use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::{
    num::NonZeroUsize,
    ops::{Bound, Range},
};

use bitflags::bitflags;
use common::config::{LOW_ADDRESS_END, MEMORY_END, MMAP_START, MMIO, PAGE_OFFSET_MASK, PA_TO_VA};
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::{MmapFlags, MmapProt},
};
use goblin::elf::{
    program_header::{PF_R, PF_W, PF_X, PT_LOAD},
    Elf,
};
use klocks::Lazy;
use smallvec::SmallVec;
use triomphe::Arc;
use virtio_drivers::PAGE_SIZE;
use vm_area::AreaType;

use self::{
    init_stack::{StackInitCtx, AT_PAGESZ},
    vm_area::FramedVmArea,
};
use super::{
    kernel_pa_to_va, kernel_vpn_to_ppn, PTEFlags, PageTable, PhysAddr, VirtAddr, VirtPageNum,
};
use crate::fs::DynPagedInode;

pub mod init_stack;
pub mod page_table;
pub mod vm_area;

pub static KERNEL_SPACE: Lazy<MemorySpace> = Lazy::new(MemorySpace::new_kernel);

/// 进程的内存地址空间
pub struct MemorySpace {
    page_table: PageTable,
    // 起始 vpn 映射到 VmArea
    user_areas: BTreeMap<VirtPageNum, FramedVmArea>,
}

impl MemorySpace {
    fn new_bare() -> Self {
        Self {
            page_table: PageTable::with_root(),
            user_areas: BTreeMap::new(),
        }
    }

    fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();

        unsafe {
            memory_set.kernel_map(
                VirtAddr(stext as usize),
                VirtAddr(etext as usize),
                MapPermission::R | MapPermission::X | MapPermission::G,
            );

            // 注：旧版的 Linux 中，text 段和 rodata 段是合并在一起的，这样可以减少一次映射
            // 新版本则独立开来了，参考 https://stackoverflow.com/questions/44938745/rodata-section-loaded-in-executable-page
            memory_set.kernel_map(
                VirtAddr(srodata as usize),
                VirtAddr(erodata as usize),
                MapPermission::R | MapPermission::G,
            );

            memory_set.kernel_map(
                VirtAddr(sdata as usize),
                VirtAddr(edata as usize),
                MapPermission::R | MapPermission::W | MapPermission::G,
            );

            memory_set.kernel_map(
                VirtAddr(sstack as usize),
                VirtAddr(estack as usize),
                MapPermission::R | MapPermission::W | MapPermission::G,
            );

            memory_set.kernel_map(
                VirtAddr(sbss as usize),
                VirtAddr(ebss as usize),
                MapPermission::R | MapPermission::W | MapPermission::G,
            );

            // TODO: 这里也许可以 Huge Page 映射？
            memory_set.kernel_map(
                VirtAddr(ekernel as usize),
                kernel_pa_to_va(PhysAddr(MEMORY_END)),
                MapPermission::R | MapPermission::W | MapPermission::G,
            );

            // MMIO 映射
            for &(start, len) in MMIO {
                memory_set.kernel_map(
                    kernel_pa_to_va(PhysAddr(start)),
                    kernel_pa_to_va(PhysAddr(start + len)),
                    MapPermission::R | MapPermission::W | MapPermission::G,
                );
            }
        }

        memory_set
    }

    /// 返回的 `VirtAddr` 是 `elf_end`，用户的 brk 放在这之后
    pub fn new_user(elf: &Elf<'_>, elf_data: &[u8]) -> (Self, VirtAddr) {
        let mut memory_set = Self::new_bare();
        let elf_end = memory_set.load_sections(elf, elf_data);
        memory_set.map_kernel_areas();
        (memory_set, elf_end)
    }

    /// 从当前用户地址空间复制一个地址空间
    pub fn from_other(user_space: &Self) -> Self {
        let mut memory_set = Self::new_bare();
        for src_area in user_space.user_areas.values() {
            let vpn_range = src_area.vpn_range();
            unsafe {
                if let Some(backed_file) = src_area.backed_file() {
                    memory_set.user_map_with_file(
                        vpn_range.clone(),
                        src_area.perm(),
                        Arc::clone(backed_file),
                        src_area.backed_file_page_id(),
                    );
                } else {
                    memory_set.user_map(vpn_range.clone(), src_area.perm());
                }
            }
            let dst_area = memory_set
                .user_areas
                .get_mut(&vpn_range.start)
                .expect("just insert above");
            for (&vpn, src_frame) in src_area.unbacked_map() {
                let mut dst_frame = dst_area
                    .ensure_allocated(vpn, &mut memory_set.page_table)
                    .frame_mut();
                dst_frame.copy_from(&src_frame.frame());
            }
        }
        memory_set.map_kernel_areas();
        memory_set
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
                trace!("load vm area {:#x}..{:#x}", start_va.0, end_va.0);
                unsafe {
                    self.user_map_with_data(
                        start_va.vpn_floor()..end_va.vpn_ceil(),
                        map_perm,
                        &elf_data[ph.file_range()],
                        start_offset,
                    );
                }
            }
        }
        elf_end
    }

    /// 映射高地址中的内核段，注意不持有它们的所有权
    fn map_kernel_areas(&mut self) {
        self.page_table.map_kernel_areas();
    }

    /// 需保证 `heap_start` < `new_end`，且还有足够的虚地址和物理空间可以映射
    pub fn set_user_brk(&mut self, heap_start: VirtPageNum, new_end: VirtPageNum) {
        // TODO: [low] 其实这里还需要考虑堆区之上有没有已经映射过的地址吧？
        // 堆区已经映射过了，就扩张或者收缩。否则插入堆区
        // 注意扩张和插入的堆区都是懒分配的
        if let Some(map_area) = self.user_areas.get_mut(&heap_start) {
            if new_end <= map_area.vpn_range().end {
                map_area.shrink(new_end, &mut self.page_table);
                flush_tlb(None);
            } else {
                map_area.expand(new_end);
            }
        } else {
            let perm = MapPermission::R | MapPermission::W | MapPermission::U;
            unsafe {
                self.user_map(heap_start..new_end, perm);
            }
        }
    }

    /// 尝试根据 `va_range` 进行映射
    pub fn try_map(
        &mut self,
        addr: usize,
        len: NonZeroUsize,
        perm: MapPermission,
        flags: MmapFlags,
    ) -> KResult<VirtPageNum> {
        let vpn_range = self.try_find_mmap_area(addr, len, flags)?;
        // SAFETY: 上面寻找映射区域的函数保证不会返回重叠的区域
        unsafe {
            self.user_map(vpn_range, perm);
        }
        Err(errno::ENOMEM)
    }

    /// 尝试根据 `va_range` 进行映射
    pub fn try_map_file(
        &mut self,
        addr: usize,
        len: NonZeroUsize,
        perm: MapPermission,
        flags: MmapFlags,
        file: Arc<DynPagedInode>,
        file_page_id: usize,
    ) -> KResult<VirtPageNum> {
        let vpn_range = self.try_find_mmap_area(addr, len, flags)?;
        // SAFETY: 上面寻找映射区域的函数保证不会返回重叠的区域
        unsafe {
            self.user_map_with_file(vpn_range.clone(), perm, file, file_page_id);
        }
        flush_tlb(None);
        Ok(vpn_range.start)
    }

    fn try_find_mmap_area(
        &mut self,
        addr: usize,
        len: NonZeroUsize,
        flags: MmapFlags,
    ) -> KResult<Range<VirtPageNum>> {
        if flags.contains(MmapFlags::MAP_FIXED) {
            if addr & PAGE_OFFSET_MASK != 0 {
                return Err(errno::EINVAL);
            }
            // TODO: [mid] 实现 fixed 映射
            error!("fixed map unsupported");
            return Err(errno::UNSUPPORTED);
        }
        // 尝试找到一个合适的段来映射
        let mut start = VirtAddr(MMAP_START).max(VirtAddr(addr)).vpn_floor();
        let mut cursor = self.user_areas.lower_bound(Bound::Excluded(&start));
        // `MMAP_START` 左侧的一个 area 有可能恰好包含了它，所以需要特判一下
        if let Some((_, area)) = cursor.peek_prev() {
            start = start.max(area.vpn_range().end);
        }
        while let Some((_, area)) = cursor.next() {
            let end_va = start.page_start() + len.get();
            if end_va <= area.vpn_range().start.page_start() {
                return Ok(start..end_va.vpn_ceil());
            }
            start = area.vpn_range().end;
        }
        // 最后一个 area 末尾到低地址末端也可以试一下
        let end_va = start.page_start() + len.get();
        if end_va.0 <= LOW_ADDRESS_END {
            return Ok(start..end_va.vpn_ceil());
        }
        Err(errno::ENOMEM)
    }

    /// 将 `va_range` 范围内的所有页取消映射。有可能导致某个 area 被部分截断
    pub fn unmap(&mut self, va_range: Range<VirtAddr>) {
        let vpn_range = va_range.start.vpn_floor()..va_range.end.vpn_ceil();
        let mut cursor = self
            .user_areas
            .lower_bound(Bound::Included(&vpn_range.start));
        cursor.prev();

        let mut to_unmap = SmallVec::<[VirtPageNum; 4]>::new();

        while let Some((_, area)) = cursor.next() {
            let area_vpn_range = area.vpn_range();
            if vpn_range.start <= area_vpn_range.start && area_vpn_range.end <= vpn_range.end {
                to_unmap.push(area_vpn_range.start);
                // area 被完全包含在内
            } else if area_vpn_range.end <= vpn_range.start {
                // area 完全在该区域左侧
                continue;
            } else if area_vpn_range.start >= vpn_range.end {
                // area 完全在该区域右侧
                break;
            } else {
                // area 部分包含在内
                todo!("[mid] impl partially contianed unmap");
            }
        }

        for vpn in to_unmap {
            let mut area = self.user_areas.remove(&vpn).unwrap();
            area.unmap(&mut self.page_table);
        }

        flush_tlb(None);
    }

    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some(mut area) = self.user_areas.remove(&start_vpn) {
            area.unmap(&mut self.page_table);
        }
    }

    /// 映射一段用户的帧映射内存区域。但并不立刻分配内存
    ///
    /// # Safety
    ///
    /// 需要保证该虚拟地址区域未被映射
    pub unsafe fn user_map(&mut self, vpn_range: Range<VirtPageNum>, perm: MapPermission) {
        let map_area = FramedVmArea::new(vpn_range, perm, AreaType::Lazy);
        self.user_areas.insert(map_area.vpn_range().start, map_area);
    }

    /// 映射一段用户的帧映射内存区域。但并不立刻分配内存
    ///
    /// # Safety
    ///
    /// 需要保证该虚拟地址区域未被映射
    pub unsafe fn user_map_with_file(
        &mut self,
        vpn_range: Range<VirtPageNum>,
        perm: MapPermission,
        file: Arc<DynPagedInode>,
        file_page_id: usize,
    ) {
        let mut map_area = FramedVmArea::new(vpn_range.clone(), perm, AreaType::Mmap);
        map_area.init_backed_file(file, file_page_id, &mut self.page_table);
        self.user_areas.insert(map_area.vpn_range().start, map_area);
    }

    /// `page_offset` 是数据在页中开始的偏移
    ///
    /// # Safety
    ///
    /// 需要保证该虚拟地址区域未被映射
    unsafe fn user_map_with_data(
        &mut self,
        vpn_range: Range<VirtPageNum>,
        perm: MapPermission,
        data: &[u8],
        page_offset: usize,
    ) {
        // TODO: [low] 实现 ELF 懒加载
        let mut map_area = FramedVmArea::new(vpn_range, perm, AreaType::Lazy);
        unsafe {
            map_area.map_with_data(&mut self.page_table, data, page_offset);
        }
        self.user_areas.insert(map_area.vpn_range().start, map_area);
    }

    unsafe fn kernel_map(&mut self, start_va: VirtAddr, end_va: VirtAddr, perm: MapPermission) {
        let start_vpn = start_va.vpn_floor();
        let end_vpn = end_va.vpn_ceil();
        for vpn in start_vpn..end_vpn {
            let ppn = kernel_vpn_to_ppn(vpn);
            self.page_table.map(vpn, ppn, PTEFlags::from(perm));
        }
    }

    /// 如有必要就切换页表，只在内核态调用，执行流不会跳变
    pub fn activate(&self) {
        self.page_table.activate();
    }

    pub fn recycle_user_pages(&mut self) {
        self.user_areas.clear();
        self.page_table.clear();
    }

    // TODO: [mid] 处理访存异常的时候似乎没有考虑权限？
    pub fn handle_memory_exception(&mut self, addr: usize, maybe_cow: bool) -> bool {
        if maybe_cow {
            todo!("[mid] impl cow");
        } else {
            trace!("handle page fault for {addr:#x}");
            let vpn = VirtAddr(addr).vpn_floor();
            if let Some((_, area)) = self.user_areas.range_mut(..=vpn).next_back() {
                if vpn >= area.vpn_range().end {
                    return false;
                }
                match area.area_type() {
                    AreaType::Lazy => {
                        area.ensure_allocated(vpn, &mut self.page_table);
                        flush_tlb(Some(vpn.page_start()));
                        return true;
                    }
                    AreaType::Mmap => todo!("[high] impl mmap memory"),
                }
            }
            false
        }
    }

    // 返回 `user_sp` 与 `argv_base`
    pub fn init_stack(
        &mut self,
        user_sp_vpn: VirtPageNum,
        args: Vec<CompactString>,
        envs: Vec<CompactString>,
    ) -> Option<(usize, usize)> {
        let (_, area) = self.user_areas.range_mut(..=user_sp_vpn).next_back()?;
        if user_sp_vpn > area.vpn_range().end {
            return None;
        }

        let ctx = StackInitCtx::new(
            user_sp_vpn,
            &mut self.page_table,
            args,
            envs,
            vec![(AT_PAGESZ, PAGE_SIZE)],
        );
        Some(area.init_stack_impl(ctx))
    }
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

impl From<MmapProt> for MapPermission {
    fn from(mmap_prot: MmapProt) -> Self {
        Self::from_bits_truncate((mmap_prot.bits() << 1) as u8) | MapPermission::U
    }
}

/// 刷新 tlb，可选刷新一部分，或者全部刷新
pub fn flush_tlb(vaddr: Option<VirtAddr>) {
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

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sstack();
    fn estack();
    fn sbss();
    fn ebss();
    fn ekernel();
}

pub fn log_kernel_sections() {
    info!(
        "kernel     text {:#x}..{:#x}",
        stext as usize, etext as usize
    );
    info!(
        "kernel   rodata {:#x}..{:#x}",
        srodata as usize, erodata as usize
    );
    info!(
        "kernel     data {:#x}..{:#x}",
        sdata as usize, edata as usize
    );
    info!(
        "kernel    stack {:#x}..{:#x}",
        sstack as usize, estack as usize
    );
    info!("kernel      bss {:#x}..{:#x}", sbss as usize, ebss as usize);
    info!(
        "physical memory {:#x}..{:#x}",
        ekernel as usize,
        PA_TO_VA + MEMORY_END
    );
}
