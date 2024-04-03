//! Implementation of [`VmArea`] and [`MemorySet`].

use crate::memory::kernel_ppn_to_vpn;
use crate::memory::kernel_va_to_pa;
use crate::memory::PhysAddr;
use crate::memory::PhysPageNum;
use crate::memory::VirtAddr;
use crate::memory::VirtPageNum;
use alloc::collections::BTreeMap;
use bitflags::bitflags;
use common::config::{LOW_ADDRESS_END, MEMORY_END, MMIO, PAGE_SIZE};
use core::ops::Range;
use defines::error::Result;
use goblin::elf::Elf;
use goblin::elf64::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use klocks::Lazy;
use riscv::register::satp;

use crate::memory::PageTable;

use super::vm_area::VmArea;

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
    user_areas: BTreeMap<VirtPageNum, VmArea>,
}

impl MemorySpace {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::with_root(),
            user_areas: BTreeMap::new(),
        }
    }

    fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();

        memory_set.push(
            VmArea::kernel_map(
                kernel_va_to_pa(VirtAddr(stext as usize)),
                kernel_va_to_pa(VirtAddr(etext as usize)),
                MapPermission::R | MapPermission::X | MapPermission::G,
            ),
            None,
        );

        // 注：旧版的 Linux 中，text 段和 rodata 段是合并在一起的，这样可以减少一次映射
        // 新版本则独立开来了，参考 https://stackoverflow.com/questions/44938745/rodata-section-loaded-in-executable-page
        memory_set.push(
            VmArea::kernel_map(
                kernel_va_to_pa(VirtAddr(srodata as usize)),
                kernel_va_to_pa(VirtAddr(erodata as usize)),
                MapPermission::R | MapPermission::G,
            ),
            None,
        );

        // .data 段和 .bss 段的访问限制相同，所以可以放到一起
        memory_set.push(
            VmArea::kernel_map(
                kernel_va_to_pa(VirtAddr(sdata as usize)),
                kernel_va_to_pa(VirtAddr(ebss as usize)),
                MapPermission::R | MapPermission::W | MapPermission::G,
            ),
            None,
        );

        // TODO: 这里也许可以 Huge Page 映射？
        memory_set.push(
            VmArea::kernel_map(
                kernel_va_to_pa(VirtAddr(ekernel as usize)),
                PhysAddr(MEMORY_END),
                MapPermission::R | MapPermission::W | MapPermission::G,
            ),
            None,
        );

        // MMIO 映射
        for &(start, len) in MMIO {
            memory_set.push(
                VmArea::kernel_map(
                    PhysAddr(start),
                    PhysAddr(start + len),
                    MapPermission::R | MapPermission::W | MapPermission::G,
                ),
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
                trace!("load vm area {:#x}..{:#x}", start_va.0, end_va.0);
                let vm_area = VmArea::new_framed(start_va, end_va, map_perm);
                self.push(vm_area, Some((&elf_data[ph.file_range()], start_offset)));
            }
        }
        elf_end
    }

    /// 映射高地址中的内核段，注意不持有它们的所有权
    pub fn map_kernel_areas(&mut self) {
        // 用户地址空间中，高地址是内核的部分
        // 具体而言，就是 [0xffff_ffff_8000_000, 0xffff_ffff_ffff_fff]
        // 以及 [0xffff_ffff_0000_0000, 0xffff_ffff_3fff_ffff]（MMIO 所在的大页）
        // 也就是内核根页表的第 508、510、511 项
        unsafe {
            // 这些需要映射到用户的页表中
            for line in [508, 510, 511] {
                let user_pte = self.page_table.root_pte_mut(line);
                let kernel_pte = KERNEL_SPACE.page_table.root_pte(line);
                user_pte.bits = kernel_pte.bits;
            }
        }
    }

    /// 从另一地址空间复制
    pub fn from_existed_user(user_space: &Self) -> Self {
        let mut memory_set = Self::new_bare();
        for (_, area) in user_space.user_areas.iter() {
            let new_area = VmArea::from_another(area);
            memory_set.push(new_area, None);
            // 从该地址空间复制数据
            for vpn in area.vpn_range.clone() {
                let src_ppn = user_space.translate(vpn).unwrap();
                let dst_ppn = memory_set.translate(vpn).unwrap();
                unsafe {
                    kernel_ppn_to_vpn(dst_ppn)
                        .as_page_bytes_mut()
                        .copy_from_slice(kernel_ppn_to_vpn(src_ppn).as_page_bytes());
                }
            }
        }
        // 已存在的地址空间高地址部分也已经映射了，所以从它那里复制内核空间也是可以的
        memory_set.map_kernel_areas();
        memory_set
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
    ) -> Result<isize> {
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

    /// 插入帧映射的一个内存段，假定是不会造成冲突的
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        perm: MapPermission,
    ) {
        self.push(VmArea::new_framed(start_va, end_va, perm), None);
    }

    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some(mut area) = self.user_areas.remove(&start_vpn) {
            area.unmap(&mut self.page_table);
        }
    }

    /// `page_offset` 是数据在页中开始的偏移
    pub fn push(&mut self, mut map_area: VmArea, data_with_page_offset: Option<(&[u8], usize)>) {
        map_area.map(&mut self.page_table);
        if let Some((data, offset)) = data_with_page_offset {
            map_area.copy_data(&mut self.page_table, offset, data);
        }
        self.user_areas.insert(map_area.vpn_range.start, map_area);
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

    // pub fn recycle_all_pages(&mut self) {
    //     self.areas.clear();
    // }

    /// 只回收进程低 256GiB 部分的页面，也就是用户进程专属的页（包括页表）
    pub fn recycle_user_pages(&mut self) {
        // TODO: 等等，Memory.areas 中是不是其实只存放了用户地址的映射？
        // 也就是只保留高地址的空间
        self.user_areas
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
