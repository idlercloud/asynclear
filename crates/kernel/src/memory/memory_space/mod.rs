use core::ops::Range;

use alloc::{collections::BTreeMap, vec, vec::Vec};
use bitflags::bitflags;
use common::config::{MEMORY_END, MMIO};
use compact_str::CompactString;
use defines::error::KResult;
use goblin::elf::{
    program_header::{PF_R, PF_W, PF_X, PT_LOAD},
    Elf,
};
use klocks::Lazy;
use virtio_drivers::PAGE_SIZE;
use vm_area::AreaType;

use self::{
    init_stack::{StackInitCtx, AT_PAGESZ},
    vm_area::FramedVmArea,
};

use super::{
    kernel_pa_to_va, kernel_vpn_to_ppn, PTEFlags, PageTable, PhysAddr, VirtAddr, VirtPageNum,
};

pub mod init_stack;
pub mod page_table;
pub mod vm_area;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss();
    fn ebss();
    fn ekernel();

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

pub fn log_kernel_sections() {
    info!("kernel text {:#x}..{:#x}", stext as usize, etext as usize);
    info!(
        "kernel rodata {:#x}..{:#x}",
        srodata as usize, erodata as usize
    );
    info!("kernel data {:#x}..{:#x}", sdata as usize, edata as usize);
    info!("kernel bss {:#x}..{:#x}", sbss as usize, ebss as usize);
    info!("physical memory {:#x}..{:#x}", ekernel as usize, MEMORY_END);
}

pub static KERNEL_SPACE: Lazy<MemorySpace> = Lazy::new(MemorySpace::new_kernel);

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

        // .data 段和 .bss 段的访问限制相同，所以可以放到一起
        memory_set.kernel_map(
            VirtAddr(sdata as usize),
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
                memory_set.user_map(
                    vpn_range.start.page_start(),
                    vpn_range.end.page_start(),
                    src_area.perm(),
                    src_area.area_type(),
                );
            };
            let dst_area = memory_set
                .user_areas
                .get_mut(&vpn_range.start)
                .expect("just insert above");
            for vpn in vpn_range {
                if let Some(src_frame) = src_area.mapped_frame(vpn) {
                    let mut dst_frame = dst_area
                        .ensure_allocated(vpn, &mut memory_set.page_table)
                        .frame();
                    dst_frame.copy_from(&src_frame);
                }
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
                        start_va..end_va,
                        map_perm,
                        AreaType::Elf,
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

    // /// 需保证 `heap_start` < `new_vpn`，且还有足够的虚地址和物理空间可以映射
    // pub fn set_user_brk(&mut self, new_end: VirtPageNum, heap_start: VirtPageNum) {
    //     // 堆区已经映射过了，就扩张或者收缩。否则插入堆区
    //     if let Some(map_area) = self.areas.get_mut(&heap_start) {
    //         let curr_vpn = map_area.end();
    //         if curr_vpn >= new_end {
    //             map_area.shrink(new_end, &mut self.page_table);
    //         } else {
    //             map_area.expand(new_end, &mut self.page_table);
    //         }
    //     } else {
    //         self.insert_framed_area(
    //             heap_start,
    //             new_end,
    //             MapPermission::R | MapPermission::W | MapPermission::U,
    //         );
    //     }
    // }

    /// 尝试根据 `va_range` 进行映射
    pub fn try_map(
        &mut self,
        _va_range: Range<VirtAddr>,
        _perm: MapPermission,
        _fixed: bool,
    ) -> KResult<isize> {
        todo!("[mid] impl mmap with page cache")
        // if fixed {
        //     // TODO: 应当 unmap 与其相交的部分。不过，如果是一些不该 unmap 的区域，是否该返回错误？
        //     self.insert_framed_area(va_range.start, va_range.end, perm);
        //     Ok(va_range.start.0 as isize)
        // } else {
        //     // 尝试找到一个合适的段来映射
        //     let mut start = VirtAddr(MMAP_START);
        //     let len = va_range.end.0 - va_range.start.0;
        //     for area in self.areas.values() {
        //         // 要控制住不溢出低地址空间的上限
        //         if area.vpn_range.start.page_start() > start
        //             && start + len <= VirtAddr(LOW_ADDRESS_END)
        //         {
        //             // 找到可映射的段
        //             if start + len <= area.vpn_range.start {
        //                 // TODO: 匿名映射的话，按照约定应当全部初始化为 0
        //                 self.insert_framed_area(start, start + len, perm);
        //                 return Ok(start.page_start().0 as isize);
        //             }
        //             start = area.vpn_range.end;
        //         }
        //     }
        //     Err(errno::ENOMEM)
        // }
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
    pub unsafe fn user_map(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        perm: MapPermission,
        area_type: AreaType,
    ) {
        let map_area = FramedVmArea::new(start_va..end_va, perm, area_type);
        self.user_areas.insert(map_area.vpn_range().start, map_area);
    }

    /// `page_offset` 是数据在页中开始的偏移
    ///
    /// # Safety
    ///
    /// 需要保证该虚拟地址区域未被映射
    unsafe fn user_map_with_data(
        &mut self,
        va_range: Range<VirtAddr>,
        perm: MapPermission,
        area_type: AreaType,
        data: &[u8],
        page_offset: usize,
    ) {
        let mut map_area = FramedVmArea::new(va_range, perm, area_type);
        unsafe {
            map_area.map_with_data(&mut self.page_table, data, page_offset);
        }
        self.user_areas.insert(map_area.vpn_range().start, map_area);
    }

    fn kernel_map(&mut self, start_va: VirtAddr, end_va: VirtAddr, perm: MapPermission) {
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

    pub fn handle_memory_exception(&mut self, addr: usize, maybe_cow: bool) -> bool {
        if maybe_cow {
            todo!("[mid] impl cow");
        } else {
            debug!("handle page fault for {addr:x}");
            let vpn = VirtAddr(addr).vpn_floor();
            if let Some((_, area)) = self.user_areas.range_mut(..=vpn).next_back() {
                if vpn >= area.vpn_range().end {
                    return false;
                }
                match area.area_type() {
                    AreaType::Stack => {
                        area.ensure_allocated(vpn, &mut self.page_table);
                        flush_tlb(Some(vpn.page_start()));
                        return true;
                    }
                    AreaType::Elf => todo!("[mid] impl elf backed memory"),
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
