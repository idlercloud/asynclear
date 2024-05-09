//! Implementation of [`PageTableEntry`] and [`PageTable`].

use alloc::vec::Vec;

use bitflags::*;
use common::config::{PAGE_SIZE, PTE_PER_PAGE};
use riscv::register::satp;

use super::{flush_tlb, KERNEL_SPACE};
use crate::memory::{frame_allocator::Frame, MapPermission, PhysPageNum, VirtPageNum};

bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u16 {
        const V =   1 << 0;
        const R =   1 << 1;
        const W =   1 << 2;
        const X =   1 << 3;
        const U =   1 << 4;
        const G =   1 << 5;
        const A =   1 << 6;
        const D =   1 << 7;
        const COW = 1 << 8;
    }
}

impl From<MapPermission> for PTEFlags {
    fn from(mp: MapPermission) -> Self {
        Self::from_bits_truncate(mp.bits() as u16)
    }
}

/// page table entry structure
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct PageTableEntry {
    bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits() as usize,
        }
    }

    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }

    pub fn ppn(&self) -> PhysPageNum {
        const LOW_44_MASK: usize = (1 << 44) - 1;
        PhysPageNum((self.bits >> 10) & LOW_44_MASK)
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits as u16)
    }

    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }
}

/// 页表，其内跟踪了页表所占用的帧，页表释放时，释放这些帧
pub struct PageTable {
    root_frame: Frame,
    frames: Vec<Frame>,
}

// 假定创建和映射时不会导致内存不足
impl PageTable {
    /// 注意，创建时会分配一个根页表的帧
    pub(super) fn with_root() -> Self {
        let frame = Frame::alloc().unwrap();
        PageTable {
            root_frame: frame,
            frames: Vec::new(),
        }
    }

    /// 释放根页表之外的其他页表，并清理根页表。
    pub(super) fn clear(&mut self) {
        self.frames.truncate(1);
        // 根页表要处理下，把用户地址的页表项去除，以防已经回收的页仍然能被访问
        // 回收了用户页表的进程不应该去访问用户数据了。因此不考虑 TLB 应该也没问题
        self.root_frame.as_page_bytes_mut()[0..PAGE_SIZE / 2].fill(0);
    }

    fn root_pte(&self, line: usize) -> &PageTableEntry {
        // SAFETY: 根页表当然存放 PTE
        unsafe { &self.root_frame.as_page_ptes()[line] }
    }

    fn root_pte_mut(&mut self, line: usize) -> &mut PageTableEntry {
        // SAFETY: 根页表当然存放 PTE
        unsafe { &mut self.root_frame.as_page_ptes_mut()[line] }
    }

    pub(super) fn map_kernel_areas(&mut self) {
        // 用户地址空间中，高地址是内核的部分
        // 具体而言，就是 [0xffff_ffff_8000_000, 0xffff_ffff_ffff_fff]
        // 以及 [0xffff_ffff_0000_0000, 0xffff_ffff_3fff_ffff]（MMIO 所在的大页）
        // 也就是内核根页表的第 508、510、511 项
        // 这些需要映射到用户的页表中
        for line in [508, 510, 511] {
            let user_pte = self.root_pte_mut(line);
            let kernel_pte = KERNEL_SPACE.page_table.root_pte(line);
            *user_pte = *kernel_pte;
        }
    }

    /// 找到 `vpn` 对应的叶子页表项。注意不保证该页表项 valid，需调用方自己修改
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_frame.ppn();
        let mut ret: Option<&mut PageTableEntry> = None;
        for (i, &idx) in idxs.iter().enumerate() {
            // SAFETY: 页表中指定的 ppn 必然已经分配；且持有着锁，因此不会 alias
            let pte = unsafe { &mut Frame::view(ppn).as_page_ptes_mut()[idx] };
            // 这里假定为 3 级页表
            if i == 2 {
                ret = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = Frame::alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn(), PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        ret
    }

    pub(super) fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        debug_assert!(
            !pte.is_valid(),
            "vpn {:#x?} is mapped before mapping",
            vpn.0
        );
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    pub(super) fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte_create(vpn).unwrap();
        debug_assert!(pte.is_valid(), "vpn {vpn:x?} is invalid before unmapping");
        *pte = PageTableEntry::empty();
    }

    fn token(&self) -> usize {
        (satp::Mode::Sv39 as usize) << 60 | self.root_frame.ppn().0
    }

    pub fn activate(&self) {
        let old_root = satp::read().bits();
        let new_root = self.token();
        if new_root != old_root {
            satp::write(new_root);
            flush_tlb(None);
        }
    }
}

#[extend::ext(name = AsPtes)]
impl Frame {
    /// # Safety
    ///
    /// 需要确保该页确实存放页表
    unsafe fn as_page_ptes<'a>(&self) -> &'a [PageTableEntry; PTE_PER_PAGE] {
        unsafe { self.as_ref_at(0) }
    }

    /// # Safety
    ///
    /// 需要确保该页确实存放页表
    unsafe fn as_page_ptes_mut<'a>(&mut self) -> &'a mut [PageTableEntry; PTE_PER_PAGE] {
        unsafe { self.as_mut_at(0) }
    }
}
