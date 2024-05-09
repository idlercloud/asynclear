use alloc::vec::Vec;

use common::config::{PAGE_SIZE, PTR_SIZE};
use compact_str::CompactString;
use triomphe::Arc;

use crate::memory::{FramedVmArea, Page, PageTable, VirtAddr, VirtPageNum};

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

impl<'a, 'b> FramedVmArea {
    /// 返回 `user_sp` 与 `argv_base`
    pub(super) fn init_stack_impl(&'b mut self, mut ctx: StackInitCtx<'a>) -> (usize, usize) {
        let argc = ctx.args.len();
        let ctx = &mut ctx;
        self.push_usize(0, ctx);
        // 这里应放入 16 字节的随机数。目前实现依赖运行时间
        // 据 Hacker News 所说，它是 "used to construct stack canaries and function
        // pointer encryption keys" 参考 https://news.ycombinator.com/item?id=24113026
        self.push_usize(riscv_time::get_time(), ctx);
        self.push_usize(riscv_time::get_time(), ctx);
        let random_pos = ctx.user_sp;
        let envs: Vec<usize> = core::mem::take(&mut ctx.envs)
            .into_iter()
            .map(|env| self.push_str(&env, ctx))
            .collect();
        self.push_usize(0, ctx);
        let argv: Vec<usize> = core::mem::take(&mut ctx.args)
            .into_iter()
            .map(|arg| self.push_str(&arg, ctx))
            .collect();
        // 清空低 3 位，也就是对齐到 8 字节，这个过程不会越过页边界
        ctx.user_sp &= !0b111;
        // AT_NULL 的 auxv（auxv 是键值对）
        self.push_usize(0, ctx);
        self.push_usize(0, ctx);

        // 辅助向量
        // 随机串的地址
        self.push_usize(AT_RANDOM as usize, ctx);
        self.push_usize(random_pos, ctx);
        // type 在低地址，而 value 在高地址
        for (type_, value) in core::mem::take(&mut ctx.auxv) {
            self.push_usize(value, ctx);
            self.push_usize(type_ as usize, ctx);
        }

        // 环境变量指针向量
        self.push_usize(0, ctx);
        self.push_ptrs(&envs, ctx);

        // 参数指针向量
        self.push_usize(0, ctx);
        self.push_ptrs(&argv, ctx);
        let argv_base = ctx.user_sp;

        // 推入 argc
        self.push_usize(argc, ctx);
        (ctx.user_sp, argv_base)
    }

    /// `user_sp` 和 `user_sp_kernel_va` 向下移动，如果跨越页边界，则重新翻译
    /// `user_sp_kernel_va`
    fn sp_down(&'b mut self, len: usize, ctx: &mut StackInitCtx<'a>) {
        ctx.user_sp -= len;

        if (ctx.user_sp + len) % PAGE_SIZE == 0 {
            let vpn = VirtAddr(ctx.user_sp).vpn_floor();
            ctx.page = Some(Arc::clone(self.ensure_allocated(vpn, ctx.page_table)));
        }
    }

    fn push_str(&'b mut self, s: &str, ctx: &mut StackInitCtx<'a>) -> usize {
        // 按规范而言，这里的字符串都是符合 c 标准的字符串，末尾为 `\0`
        self.push_byte(0, ctx);
        for &byte in s.as_bytes().iter().rev() {
            self.push_byte(byte, ctx);
        }
        ctx.user_sp
    }

    fn push_ptrs(&'b mut self, ptrs: &[usize], ctx: &mut StackInitCtx<'a>) {
        for &ptr in ptrs.iter().rev() {
            self.push_usize(ptr, ctx);
        }
    }

    fn push_byte(&'b mut self, byte: u8, ctx: &mut StackInitCtx<'a>) {
        self.sp_down(1, ctx);
        unsafe {
            // SAFETY: sp_down 之后 frame 一定被初始化了
            let mut frame = ctx.page.as_mut().unwrap_unchecked().frame_mut();
            *frame.as_mut_at(VirtAddr(ctx.user_sp).page_offset()) = byte;
        }
    }

    fn push_usize(&'b mut self, num: usize, ctx: &mut StackInitCtx<'a>) {
        self.sp_down(PTR_SIZE, ctx);
        unsafe {
            let mut frame = ctx.page.as_mut().unwrap_unchecked().frame_mut();
            *frame.as_mut_at(VirtAddr(ctx.user_sp).page_offset()) = num;
        }
    }
}

pub(super) struct StackInitCtx<'a> {
    /// 用户地址空间的 sp
    user_sp: usize,
    page_table: &'a mut PageTable,
    args: Vec<CompactString>,
    envs: Vec<CompactString>,
    auxv: Vec<(u8, usize)>,
    page: Option<Arc<Page>>,
}

impl<'a> StackInitCtx<'a> {
    /// 如果 `user_pt` 不为 `None`，则使用该 `PageTable` 转换的地址
    pub fn new(
        user_sp_page: VirtPageNum,
        page_table: &'a mut PageTable,
        args: Vec<CompactString>,
        envs: Vec<CompactString>,
        auxv: Vec<(u8, usize)>,
    ) -> Self {
        // 用户 sp 最初应该是对齐到页边界的
        Self {
            user_sp: user_sp_page.page_start().0,
            page_table,
            args,
            envs,
            auxv,
            page: None,
        }
    }
}
