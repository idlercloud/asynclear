use riscv::register::sstatus::{self, Sstatus, SPP};

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
    pub sstatus: Sstatus,
    /// 发生 trap 时的 pc 值。一般而言从 user trap 返回就是回到它
    pub sepc: usize,
    pub kernel_sp: usize,
    /// TODO: 这个是否其实永远是同一个值，即 [`super::trap_return`] 中最后的指令？而且是否保存了两遍？
    pub kernel_ra: usize,
    /// 内核的 tp 存放了 `local_hart` 的地址
    pub kernel_tp: usize,
    /// s0~s11。原因见 [`super::trap_return`]
    pub kernel_s: [usize; 12],
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.user_regs[1] = sp;
    }

    /// 用户应用初始化时的 `TrapContext`。
    ///
    /// 从内核返回后，会在指定的 `sp` 上从指定的 `entry` 开始运行
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        // 即将返回用户态，因此 `spp` 设为 `SPP::USER`
        sstatus.set_spp(SPP::User);
        let kernel_tp: usize;
        unsafe {
            core::arch::asm!("mv {}, tp", out(reg) kernel_tp);
        }
        let mut cx = Self {
            user_regs: [0; 31],
            sstatus,
            sepc: entry,
            // 下面这些内核相关的寄存器会在返回用户态时保存
            kernel_sp: 0,
            kernel_ra: 0,
            kernel_s: [0; 12],
            kernel_tp,
        };
        cx.set_sp(sp);
        cx
    }
}
