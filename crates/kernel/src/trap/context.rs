use core::arch::asm;

#[repr(C)]
#[derive(Clone)]
/// 该结构体保存了通用寄存器、sstatus、sepc 等
pub struct TrapContext {
    /// 不包括 x0(zero)，因为 x0 恒定为 0
    pub user_regs: [usize; 31],
    /// sstatus 存放一些状态。包括且不限于
    ///
    /// - 当前全局中断使能 (SIE)、trap 发生之前的全局中断使能 (SPIE)。
    /// - trap 之前的权限模式 (SPP)
    /// - 是否允许读取用户数据 (SUM)
    pub sstatus: usize,
    /// 发生 trap 时的 pc 值。一般而言从 user trap 返回就是回到它
    pub sepc: usize,
    pub kernel_sp: usize,
    pub kernel_ra: usize,
    /// 内核的 tp 存放了 `local_hart` 的地址
    pub kernel_tp: usize,
    /// s0~s11。原因见 [`super::trap_return`]
    pub kernel_s: [usize; 12],
}

impl TrapContext {
    pub fn sp(&self) -> usize {
        self.user_regs[1]
    }

    pub fn sp_mut(&mut self) -> &mut usize {
        &mut self.user_regs[1]
    }

    pub fn a0_mut(&mut self) -> &mut usize {
        &mut self.user_regs[9]
    }

    pub fn a1_mut(&mut self) -> &mut usize {
        &mut self.user_regs[10]
    }

    pub fn ra_mut(&mut self) -> &mut usize {
        &mut self.user_regs[0]
    }

    /// 用户应用初始化时的 `TrapContext`。
    ///
    /// 从内核返回后，会在指定的 `sp` 上从指定的 `entry` 开始运行
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let kernel_tp: usize;
        unsafe {
            core::arch::asm!("mv {}, tp", out(reg) kernel_tp);
        }
        let mut cx = Self {
            user_regs: [0; 31],
            sstatus: app_init_sstatus(),
            sepc: entry,
            // 下面这些内核相关的寄存器会在返回用户态时保存
            kernel_sp: 0,
            kernel_ra: 0,
            kernel_s: [0; 12],
            kernel_tp,
        };
        *cx.sp_mut() = sp;
        cx
    }
}

fn app_init_sstatus() -> usize {
    let mut sstatus: usize;
    unsafe { asm!("csrr {}, sstatus", out(reg) sstatus) };
    // 关闭 SIE，因为在 `__return_to_user` 过程中是关中断的
    sstatus &= !(1 << 1);
    // SPIE 设为 1，使 `sret` 后 SIE 为 1。
    // 其实设不设似乎都一样？riscv-isa-manual 说在用户模式下 SIE 是被忽略的
    sstatus |= 1 << 5;
    // SPP 设为 User
    sstatus &= !(1 << 8);
    sstatus
}
