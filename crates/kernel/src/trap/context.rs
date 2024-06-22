use core::arch::asm;

use riscv::register::sstatus::FS;

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
    pub user_float_ctx: UserFloatContext,
}

impl TrapContext {
    pub fn sp(&self) -> usize {
        self.user_regs[1]
    }

    pub fn sp_mut(&mut self) -> &mut usize {
        &mut self.user_regs[1]
    }

    pub fn a0(&self) -> usize {
        self.user_regs[9]
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
            user_float_ctx: UserFloatContext::new(),
        };
        *cx.sp_mut() = sp;
        cx
    }

    pub fn fs(&self) -> FS {
        match (self.sstatus >> 13) & 0b11 {
            0 => FS::Off,
            1 => FS::Initial,
            2 => FS::Clean,
            3 => FS::Dirty,
            _ => unreachable!(),
        }
    }

    pub fn set_fs(&mut self, fs: FS) {
        set_fs(&mut self.sstatus, fs);
    }
}

fn set_fs(sstatus: &mut usize, fs: FS) {
    *sstatus = (*sstatus & !(0b11 << 13)) | ((fs as usize) << 13);
}

fn app_init_sstatus() -> usize {
    let mut sstatus: usize;

    unsafe { asm!("csrr {}, sstatus", out(reg) sstatus) };
    set_fs(&mut sstatus, FS::Clean);
    // 关闭 SIE，因为在 `__return_to_user` 过程中是关中断的
    sstatus &= !(1 << 1);
    // SPIE 设为 1，使 `sret` 后 SIE 为 1。
    // 其实设不设似乎都一样？riscv-isa-manual 说在用户模式下 SIE 是被忽略的
    sstatus |= 1 << 5;
    // SPP 设为 User
    sstatus &= !(1 << 8);
    sstatus
}

#[repr(C)]
#[derive(Clone)]
pub struct UserFloatContext {
    pub user_fx: [f64; 32],
    pub fcsr: u32,
    pub valid: bool,
}

impl UserFloatContext {
    pub fn new() -> Self {
        unsafe { core::mem::zeroed() }
    }

    pub fn save(&mut self) {
        unsafe {
            asm!(
                "fsd  f0,  0*8({0})",
                "fsd  f1,  1*8({0})",
                "fsd  f2,  2*8({0})",
                "fsd  f3,  3*8({0})",
                "fsd  f4,  4*8({0})",
                "fsd  f5,  5*8({0})",
                "fsd  f6,  6*8({0})",
                "fsd  f7,  7*8({0})",
                "fsd  f8,  8*8({0})",
                "fsd  f9,  9*8({0})",
                "fsd f10, 10*8({0})",
                "fsd f11, 11*8({0})",
                "fsd f12, 12*8({0})",
                "fsd f13, 13*8({0})",
                "fsd f14, 14*8({0})",
                "fsd f15, 15*8({0})",
                "fsd f16, 16*8({0})",
                "fsd f17, 17*8({0})",
                "fsd f18, 18*8({0})",
                "fsd f19, 19*8({0})",
                "fsd f20, 20*8({0})",
                "fsd f21, 21*8({0})",
                "fsd f22, 22*8({0})",
                "fsd f23, 23*8({0})",
                "fsd f24, 24*8({0})",
                "fsd f25, 25*8({0})",
                "fsd f26, 26*8({0})",
                "fsd f27, 27*8({0})",
                "fsd f28, 28*8({0})",
                "fsd f29, 29*8({0})",
                "fsd f30, 30*8({0})",
                "fsd f31, 31*8({0})",
                "csrr {1}, fcsr",
                "sw  {1}, 32*8({0})",
                in(reg) self,
                out(reg) _
            );
        };
    }

    pub fn restore(&mut self) {
        unsafe {
            asm!(
                "fld  f0,  0*8({0})",
                "fld  f1,  1*8({0})",
                "fld  f2,  2*8({0})",
                "fld  f3,  3*8({0})",
                "fld  f4,  4*8({0})",
                "fld  f5,  5*8({0})",
                "fld  f6,  6*8({0})",
                "fld  f7,  7*8({0})",
                "fld  f8,  8*8({0})",
                "fld  f9,  9*8({0})",
                "fld f10, 10*8({0})",
                "fld f11, 11*8({0})",
                "fld f12, 12*8({0})",
                "fld f13, 13*8({0})",
                "fld f14, 14*8({0})",
                "fld f15, 15*8({0})",
                "fld f16, 16*8({0})",
                "fld f17, 17*8({0})",
                "fld f18, 18*8({0})",
                "fld f19, 19*8({0})",
                "fld f20, 20*8({0})",
                "fld f21, 21*8({0})",
                "fld f22, 22*8({0})",
                "fld f23, 23*8({0})",
                "fld f24, 24*8({0})",
                "fld f25, 25*8({0})",
                "fld f26, 26*8({0})",
                "fld f27, 27*8({0})",
                "fld f28, 28*8({0})",
                "fld f29, 29*8({0})",
                "fld f30, 30*8({0})",
                "fld f31, 31*8({0})",
                "lw  {1}, 32*8({0})",
                "csrw fcsr, {1}",
                in(reg) self,
                lateout(reg) _,
            );
        }
    }
}
