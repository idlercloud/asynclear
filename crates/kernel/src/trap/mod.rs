mod context;
mod kernel_trap;

use core::{ops::ControlFlow, ptr::NonNull};

pub use context::TrapContext;
use defines::{error::errno, signal::SignalActionFlags};
use kernel_tracer::Instrument;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sie,
    sstatus::{self, FS},
    stval,
    stvec::{self, TrapMode},
};

use crate::{
    drivers::{qemu_plic::Plic, qemu_uart::UART0, InterruptSource},
    executor,
    hart::local_hart,
    memory::UserCheck,
    process::{self, exit_process},
    signal::{DefaultHandler, KSignalActionExt, KSignalSet, SignalContext, SIG_DFL, SIG_ERR, SIG_IGN},
    syscall,
    thread::Thread,
    time,
};

core::arch::global_asm!(include_str!("trap.S"));

/// 在某些情况下，如调用了 `sys_exit`，会返回 `ControlFlow::Break` 以通知结束用户线程循环
pub async fn user_trap_handler() -> ControlFlow<(), ()> {
    kernel_trap::set_kernel_trap_entry();

    // NOTE: `scause` 和 `stval` 一定要在开中断前读，因为它们会被中断覆盖
    let scause = scause::read();
    let stval = stval::read();

    unsafe {
        sstatus::set_sie();
    }

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let (syscall_id, syscall_args) = {
                let trap_context = unsafe { local_hart().curr_trap_context().as_mut() };
                // TODO: syscall 的返回位置是下一条指令，不过一定是 +4 吗？
                trap_context.sepc += 4;
                let user_regs = &mut trap_context.user_regs;
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
            };
            let result = syscall::syscall(syscall_id, syscall_args)
                .instrument(info_span!("syscall", name = defines::syscall::name(syscall_id)))
                .await;

            // 线程应当退出
            if result == errno::BREAK.as_isize() {
                ControlFlow::Break(())
            } else {
                unsafe {
                    *local_hart().curr_trap_context().as_mut().a0_mut() = result as usize;
                }
                ControlFlow::Continue(())
            }
        }

        Trap::Exception(
            e @ (Exception::StoreFault
            | Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault),
        ) => {
            let _enter = info_span!("pagefault").entered();
            let thread = local_hart().curr_thread();

            let ok = thread.process.lock_inner_with(|inner| {
                inner
                    .memory_space
                    .handle_memory_exception(stval, e == Exception::StoreFault)
            });

            if ok {
                ControlFlow::Continue(())
            } else {
                let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };
                info!("regs: {:x?}", trap_context.user_regs);
                error!(
                    "{:?} in application, bad addr = {:#x}, bad inst pc = {:#x}, core dumped.",
                    scause.cause(),
                    stval,
                    trap_context.sepc,
                );
                process::exit_process(&thread.process, -2);
                ControlFlow::Break(())
            }
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            let thread = local_hart().curr_thread();
            let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };
            info!("regs: {:x?}", trap_context.user_regs);
            error!(
                "IllegalInstruction(pc={:#x}) in application, core dumped.",
                trap_context.sepc,
            );
            process::exit_process(&thread.process, -3);
            ControlFlow::Break(())
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            {
                let _enter = debug_span!("timer_irq").entered();
                time::check_timer();
                riscv_time::set_next_trigger();
            }
            executor::yield_now().await;
            ControlFlow::Continue(())
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            let _enter = debug_span!("external_irq").entered();
            interrupt_handler();
            ControlFlow::Continue(())
        }
        _ => {
            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval,);
        }
    }
}

/// 从用户任务的内核态返回到用户态。
///
/// 注意：会切换控制流和栈
pub fn trap_return(trap_context: NonNull<TrapContext>) {
    check_signal(&local_hart().curr_thread());

    // 因为 trap entry 要切换为用户的，在回到用户态之前不能触发中断
    unsafe {
        sstatus::clear_sie();
    }
    set_user_trap_entry();

    extern "C" {
        fn __return_to_user(cx: NonNull<TrapContext>);
    }

    unsafe {
        // 对内核来说，调用 __return_to_user 返回内核态就好像一次函数调用
        // 因此编译器会将 Caller Saved 的寄存器保存下来
        // 但是 Called Saved 的寄存器很快会被覆盖，因此需要在 TrapContext 上保存下来
        __return_to_user(trap_context);
    }
}

/// 如果进程因为信号被终止了，则返回 true
pub fn check_signal(thread: &Thread) -> bool {
    let first_pending = {
        let mut inner = thread.lock_inner();
        let pendings = inner.pending_signal.intersection(!inner.signal_mask);
        let Some(first_pending) = pendings.first_pending() else {
            return false;
        };
        inner.pending_signal.remove(KSignalSet::from(first_pending));
        first_pending
    };

    debug!("handle signal {first_pending:?}");
    let action = thread
        .process
        .lock_inner_with(|inner| inner.signal_handlers.action(first_pending).clone());
    trace!(
        "handler: {:#x}, mask: {:?}, flags: {:?}, restorer: {:#x}",
        action.handler,
        action.kmask(),
        action.flags,
        action.restorer
    );

    let handler = match action.handler {
        SIG_ERR => todo!("[low] maybe there is no `SIG_ERR`"),
        SIG_DFL => match DefaultHandler::new(first_pending) {
            DefaultHandler::Terminate | DefaultHandler::CoreDump => {
                exit_process(&thread.process, (first_pending as i8).wrapping_add_unsigned(128));
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

    let old_mask = thread.lock_inner_with(|inner| {
        let old_mask = inner.signal_mask;
        inner.signal_mask.insert(action.kmask());
        if !action.flags.contains(SignalActionFlags::SA_NODEFER) {
            inner.signal_mask.set(KSignalSet::from(first_pending), true);
        }
        old_mask
    });
    let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };

    let mut signal_context = SignalContext {
        old_mask,
        old_trap_context: trap_context.clone(),
    };

    // 任何信号处理都可以视作一个新的任务，因此需要单独记录浮点数的使用
    trap_context.user_float_ctx.valid = false;
    if trap_context.fs() == FS::Dirty {
        debug!("save float ctx");
        // 进入信号处理前需要保存当前线程的浮点数上下文以便信号处理完成后恢复
        signal_context.old_trap_context.user_float_ctx.save();
        signal_context.old_trap_context.user_float_ctx.valid = true;
        trap_context.set_fs(FS::Clean);
    }

    trap_context.sepc = handler;
    *trap_context.sp_mut() = trap_context.sp() - core::mem::size_of::<SignalContext>();
    *trap_context.ra_mut() = action.restorer;
    *trap_context.a0_mut() = first_pending.to_user() as usize;

    let sp = signal_context.old_trap_context.sp() - core::mem::size_of::<SignalContext>();
    let user_ptr = (|| unsafe {
        UserCheck::new(sp as *mut SignalContext)
            .ok_or(errno::EINVAL)?
            .check_ptr_mut()
    })();
    if let Ok(user_ptr) = user_ptr {
        user_ptr.write(signal_context);
        false
    } else {
        exit_process(&thread.process, (first_pending as i8).wrapping_add_unsigned(128));
        true
    }
}

pub fn init() {
    kernel_trap::set_kernel_trap_entry();
    unsafe {
        sie::set_sext();
        sie::set_stimer();
        sstatus::set_sie();
    }
    riscv_time::set_next_trigger();
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
    // 为 0 应该说明是多个核争抢同一个中断，然后没抢到？
    if interrupt_id == 0 {
        return;
    }
    let Some(interrupt_source) = InterruptSource::from_id(interrupt_id) else {
        panic!("Unknown interrupt {interrupt_id}");
    };
    match interrupt_source {
        InterruptSource::Uart0 => UART0.handle_irq(),
        InterruptSource::VirtIO => todo!("[mid] virtio interrupt handler"),
    }
    plic.complete(context_id, interrupt_id);
}
