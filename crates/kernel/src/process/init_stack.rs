use alloc::vec::Vec;
use compact_str::CompactString;
use defines::config::{PAGE_SIZE, PTR_SIZE};
use memory::{PageTable, VirtAddr};

// use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};
// use goblin::elf::{
//     header::ET_EXEC,
//     program_header,
//     program_header::{PF_R, PF_W, PF_X, PT_LOAD},
//     Elf,
// };
// use memory::{MapArea, MapPermission, MapType, MemorySet, PageTable, VirtAddr};
// use stack::{InfoBlock, StackInit};

// use crate::process::ProcessInner;

// PH 相关和 Entry 应该是用于动态链接的，交由所谓 interpreter 解析
// PH 的起始地址
#[allow(unused)]
pub const AT_PHDR: u8 = 3;
// PH 项的大小
#[allow(unused)]
pub const AT_PHENT: u8 = 4;
// PH 的数量
#[allow(unused)]
pub const AT_PHNUM: u8 = 5;
// PAGE_SIZE 的值
pub const AT_PAGESZ: u8 = 6;
// interpreter 的基地址
#[allow(unused)]
pub const AT_BASE: u8 = 7;
// 可执行文件的程序入口
#[allow(unused)]
pub const AT_ENTRY: u8 = 9;
// 指向 16 字节随机值的地址
pub const AT_RANDOM: u8 = 25;

pub struct UserStackInit<'a> {
    /// 用户地址空间的 sp
    user_sp: usize,
    /// `user_sp` 经过页表翻译得到的虚拟地址
    user_sp_kernel_va: usize,
    user_pt: &'a PageTable,
}

pub struct UserAppInfo {
    pub args: Vec<CompactString>,
    pub envs: Vec<CompactString>,
    pub auxv: Vec<(u8, usize)>,
}

impl<'a> UserStackInit<'a> {
    pub fn new(user_sp: usize, user_pt: &'a PageTable) -> Self {
        // 用户 sp 最初应该是对齐到页边界的
        debug_assert!(user_sp % PAGE_SIZE == 0);
        Self {
            user_sp,
            user_sp_kernel_va: 0,
            user_pt,
        }
    }

    pub fn user_sp(&self) -> usize {
        self.user_sp
    }

    /// 由于用户库需要 argv 放入 a1 寄存器，这里返回一下。
    pub fn init_stack(&mut self, info_block: UserAppInfo) -> usize {
        let argc = info_block.args.len();
        self.push_usize(0);
        // 这里应放入 16 字节的随机数。目前实现依赖运行时间
        // 据 Hacker News 所说，它是 "used to construct stack canaries and function pointer encryption keys"
        // 参考 https://news.ycombinator.com/item?id=24113026
        self.push_usize(riscv_time::get_time());
        self.push_usize(riscv_time::get_time());
        let random_pos = self.user_sp;
        let envs: Vec<usize> = info_block
            .envs
            .into_iter()
            .map(|env| self.push_str(&env))
            .collect();
        self.push_usize(0);
        let argv: Vec<usize> = info_block
            .args
            .into_iter()
            .map(|arg| self.push_str(&arg))
            .collect();
        // 清空低 3 位，也就是对齐到 8 字节，这个过程不会越过页边界
        self.user_sp &= !0b111;
        self.user_sp_kernel_va &= !0b111;
        // AT_NULL 的 auxv（auxv 是键值对）
        self.push_usize(0);
        self.push_usize(0);

        // 辅助向量
        // 随机串的地址
        self.push_usize(AT_RANDOM as usize);
        self.push_usize(random_pos);
        // type 在低地址，而 value 在高地址
        for (type_, value) in info_block.auxv {
            self.push_usize(value);
            self.push_usize(type_ as usize);
        }

        // 环境变量指针向量
        self.push_usize(0);
        self.push_ptrs(&envs);

        // 参数指针向量
        self.push_usize(0);
        self.push_ptrs(&argv);
        let argv_base = self.user_sp;

        // 推入 argc
        self.push_usize(argc);
        argv_base
    }

    /// sp 和 sp_kernel_va 向下移动，如果跨越页边界，则重新翻译 sp_kernel_va
    fn sp_down(&mut self, len: usize) {
        if self.user_sp % PAGE_SIZE == 0 {
            self.user_sp -= len;
            self.user_sp_kernel_va = self.user_pt.trans_va(VirtAddr(self.user_sp)).unwrap().0;
        } else {
            self.user_sp -= len;
            self.user_sp_kernel_va -= len;
        }
    }

    fn push_str(&mut self, s: &str) -> usize {
        // 按规范而言，这里的字符串都是符合 c 标准的字符串，末尾为 `\0`
        self.push_byte(0);
        for &byte in s.as_bytes().iter().rev() {
            self.push_byte(byte);
        }
        self.user_sp
    }

    fn push_ptrs(&mut self, ptrs: &[usize]) {
        for &ptr in ptrs.iter().rev() {
            self.push_usize(ptr)
        }
    }

    fn push_byte(&mut self, byte: u8) {
        self.sp_down(1);
        unsafe {
            *VirtAddr(self.user_sp_kernel_va).as_mut() = byte;
        }
    }

    fn push_usize(&mut self, ptr: usize) {
        self.sp_down(PTR_SIZE);
        unsafe {
            *VirtAddr(self.user_sp_kernel_va).as_mut() = ptr;
        }
    }
}
