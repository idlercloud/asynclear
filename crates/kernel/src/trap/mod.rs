mod context;
mod kernel_trap;

use core::ops::ControlFlow;

pub use context::TrapContext;
use defines::{
    error::errno,
    signal::{KSignalSet, Signal, SignalActionFlags},
};
use kernel_tracer::Instrument;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sie, sstatus, stval,
    stvec::{self, TrapMode},
};
use user_check::UserCheckMut;

use crate::{
    drivers::{qemu_plic::Plic, qemu_uart::UART0, InterruptSource},
    executor,
    hart::local_hart,
    process::{self, exit_process},
    signal::{DefaultHandler, SignalContext, SIG_DFL, SIG_ERR, SIG_IGN},
    syscall, time,
};

core::arch::global_asm!(include_str!("trap.S"));

/// 在某些情况下，如调用了 `sys_exit`，会返回 `ControlFlow::Break` 以通知结束用户线程循环
pub async fn user_trap_handler() -> ControlFlow<(), ()> {
    kernel_trap::set_kernel_trap_entry();

    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // syscall 过程中可以发生内核中断
            unsafe {
                sstatus::set_sie();
            }
            let (syscall_id, syscall_args) = {
                local_hart().curr_thread().lock_inner_with(|inner| {
                    // TODO: syscall 的返回位置是下一条指令，不过一定是 +4 吗？
                    inner.trap_context.sepc += 4;
                    let user_regs = &mut inner.trap_context.user_regs;
                    let syscall_id = user_regs[16];
                    let syscall_args = [
                        user_regs[9],
                        user_regs[10],
                        user_regs[11],
                        user_regs[12],
                        user_regs[13],
                        user_regs[14],
                    ];
                    (syscall_id, syscall_args)
                })
            };
            let result = syscall::syscall(syscall_id, syscall_args)
                .instrument(info_span!(
                    "syscall",
                    name = defines::syscall::name(syscall_id)
                ))
                .await;

            // 线程应当退出
            if result == errno::BREAK.as_isize() {
                ControlFlow::Break(())
            } else {
                let thread = local_hart().curr_thread();
                thread.lock_inner_with(|inner| inner.trap_context.user_regs[9] = result as usize);
                ControlFlow::Continue(())
            }
        }

        Trap::Exception(
            e @ (Exception::StoreFault
            | Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault),
        ) => {
            let thread = local_hart().curr_thread();

            let exception_addr = stval::read();
            let ok = thread.process.lock_inner_with(|inner| {
                inner
                    .memory_space
                    .handle_memory_exception(exception_addr, e == Exception::StoreFault)
            });

            if ok {
                ControlFlow::Continue(())
            } else {
                {
                    let inner = thread.lock_inner();
                    info!("regs: {:x?}", inner.trap_context.user_regs);
                    error!(
                        "{:?} in application, bad addr = {:#x}, bad inst pc = {:#x}, core dumped.",
                        scause.cause(),
                        exception_addr,
                        inner.trap_context.sepc,
                    );
                };
                process::exit_process(&thread.process, -2);
                ControlFlow::Break(())
            }
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            let thread = local_hart().curr_thread();
            {
                let inner = thread.lock_inner();
                info!("regs: {:x?}", inner.trap_context.user_regs);
                error!(
                    "IllegalInstruction(pc={:#x}) in application, core dumped.",
                    inner.trap_context.sepc,
                );
            }
            process::exit_process(&thread.process, -3);
            ControlFlow::Break(())
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            trace!("timer interrupt");
            riscv_time::set_next_trigger();
            time::check_timer();
            executor::yield_now().await;
            ControlFlow::Continue(())
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            debug!("external interrupt");
            interrupt_handler();
            ControlFlow::Continue(())
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval::read()
            );
        }
    }
}

/// 从用户任务的内核态返回到用户态。
///
/// 注意：会切换控制流和栈
pub fn trap_return(trap_context: *mut TrapContext) {
    // 因为 trap entry 要切换为用户的，在回到用户态之前不能触发中断
    unsafe {
        sstatus::clear_sie();
    }
    trace!("enter user mode");
    set_user_trap_entry();

    check_signal();

    extern "C" {
        fn __return_to_user(cx: *mut TrapContext);
    }

    unsafe {
        // 对内核来说，调用 __return_to_user 返回内核态就好像一次函数调用
        // 因此编译器会将 Caller Saved 的寄存器保存下来
        // 但是 Called Saved 的寄存器很快会被覆盖，因此需要在 TrapContext 上保存下来
        __return_to_user(trap_context);
    }
}

/// 如果进程因为信号被终止了，则返回 true
pub fn check_signal() -> bool {
    let first_pending = {
        let thread = local_hart().curr_thread();
        let mut inner = thread.lock_inner();
        let pendings = inner.pending_signal.intersection(!inner.signal_mask);
        let Ok(first_pending) = Signal::try_from(pendings.bits().trailing_zeros() as u8) else {
            return false;
        };
        inner.pending_signal.remove(KSignalSet::from(first_pending));
        first_pending
    };

    debug!("handle signal {first_pending:?}");
    let action = local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.signal_handlers.action(first_pending).clone());
    trace!(
        "handler: {:#x}, mask: {:?}, flags: {:?}, restorer: {:#x}",
        action.handler(),
        action.mask(),
        action.flags(),
        action.restorer()
    );

    let handler = match action.handler() {
        SIG_ERR => todo!("[low] maybe there is no `SIG_ERR`"),
        SIG_DFL => match DefaultHandler::new(first_pending) {
            DefaultHandler::Terminate | DefaultHandler::CoreDump => {
                exit_process(
                    &local_hart().curr_process(),
                    (first_pending as i8).wrapping_add_unsigned(128),
                );
                // TODO:[low] 要处理 CoreDump
                return true;
            }
            DefaultHandler::Ignore => return false,
            DefaultHandler::Stop | DefaultHandler::Continue => {
                // 被信号 stop 或者 continue 都要通知 `sys_wait4()`
                todo!("[low] default handler Stop and Continue")
            }
        },
        SIG_IGN => return false,
        handler => handler,
    };

    let thread = local_hart().curr_thread();

    let (old_mask, old_trap_context) = thread.lock_inner_with(|inner| {
        let old_mask = inner.signal_mask;
        let old_trap_context = inner.trap_context.clone();
        inner.signal_mask.insert(action.mask());
        if !action.flags().contains(SignalActionFlags::SA_NODEFER) {
            inner.signal_mask.set(KSignalSet::from(first_pending), true);
        }
        let trap_context = &mut inner.trap_context;
        trap_context.sepc = handler;
        *trap_context.sp_mut() = trap_context.sp() - core::mem::size_of::<SignalContext>();
        *trap_context.ra_mut() = action.restorer();
        *trap_context.a0_mut() = first_pending as usize + 1;

        (old_mask, old_trap_context)
    });

    let signal_context = SignalContext {
        old_mask,
        old_trap_context,
    };

    let sp = signal_context.old_trap_context.sp() - core::mem::size_of::<SignalContext>();
    let Ok(mut user_ptr) = UserCheckMut::new(sp as *mut SignalContext).check_ptr_mut() else {
        // TODO:[blocked] 这里其实可以试着补救
        exit_process(
            &local_hart().curr_process(),
            (first_pending as i8).wrapping_add_unsigned(128),
        );
        return true;
    };
    *user_ptr = signal_context;

    false
}

pub fn init() {
    unsafe {
        sie::set_stimer();
        riscv_time::set_next_trigger();
        kernel_trap::set_kernel_trap_entry();
        sstatus::set_sie();
    }
}

fn set_user_trap_entry() {
    extern "C" {
        fn __trap_from_user();
    }

    unsafe {
        stvec::write(__trap_from_user as usize, TrapMode::Direct);
    }
}

fn interrupt_handler() {
    let plic = unsafe { &*Plic::mmio() };
    let hart_id = local_hart().hart_id();
    let context_id = hart_id * 2;
    let interrupt_id = plic.claim(context_id);
    let Some(interrupt_source) = InterruptSource::from_id(interrupt_id) else {
        panic!("Unknown interrupt {interrupt_id}");
    };
    match interrupt_source {
        InterruptSource::Uart0 => UART0.handle_irq(),
        InterruptSource::VirtIO => todo!("[mid] virtio interrupt handler"),
    }
    plic.complete(context_id, interrupt_id);
}
