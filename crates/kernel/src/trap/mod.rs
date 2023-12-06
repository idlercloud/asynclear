use core::ops::ControlFlow;

use defines::{error::errno, trap_context::TrapContext};
use kernel_tracer::Instrument;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc, sie, stval,
    stvec::{self, TrapMode},
};

use crate::{hart::local_hart, process, syscall, thread};

core::arch::global_asm!(include_str!("trap.S"));

/// 在某些情况下，如调用了 `sys_exit`，会返回 `ControlFlow::Break`
///
/// 以通知结束用户线程循环
pub async fn trap_handler() -> ControlFlow<(), ()> {
    set_kernel_trap_entry();

    let scause = scause::read();
    trace!("Trap happened {:?}", scause.cause());
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => unsafe {
            let mut cx = (*local_hart()).trap_context();
            // TODO: 异常的返回位置是下一条指令，不过一定是 +4 吗？
            (*cx).sepc += 4;
            // get system call return value
            let syscall_id = (*cx).user_regs[16];
            let result = syscall::syscall(
                syscall_id,
                [
                    (*cx).user_regs[9],
                    (*cx).user_regs[10],
                    (*cx).user_regs[11],
                    (*cx).user_regs[12],
                    (*cx).user_regs[13],
                    (*cx).user_regs[14],
                ],
            )
            .instrument(debug_span!("syscall", name = syscall::name(syscall_id)))
            .await;

            // 线程应当退出
            if result == errno::BREAK.as_isize() {
                ControlFlow::Break(())
            } else {
                // 如果调用了 sys_exec，那么 trap_context 有可能发生了变化，因此要重新调用一下
                cx = (*local_hart()).trap_context();
                (*cx).user_regs[9] = result as usize;
                ControlFlow::Continue(())
            }
        },

        Trap::Exception(
            Exception::StoreFault
            | Exception::StorePageFault
            | Exception::InstructionFault
            | Exception::InstructionPageFault
            | Exception::LoadFault
            | Exception::LoadPageFault,
        ) => unsafe {
            let cx = (*local_hart()).trap_context();
            info!("regs: {:x?}", (*cx).user_regs);
            error!(
                "{:?} in application, bad addr = {:#x}, bad inst pc = {:#x}, core dumped.",
                scause.cause(),
                stval,
                (*cx).sepc,
            );
            process::exit_process((*local_hart()).curr_process(), -2);
            ControlFlow::Break(())
        },
        Trap::Exception(Exception::IllegalInstruction) => unsafe {
            let cx = (*local_hart()).trap_context();
            info!("regs: {:x?}", (*cx).user_regs);
            error!(
                "IllegalInstruction(pc={:#x}) in application, core dumped.",
                (*cx).sepc,
            );
            process::exit_process((*local_hart()).curr_process(), -3);
            ControlFlow::Break(())
        },
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            riscv_time::set_next_trigger();
            thread::check_timer();
            unsafe {
                (*local_hart()).curr_thread().yield_now().await;
            }
            // TODO: 其他让出控制权的方式是否也应该以 Future 形式实现
            ControlFlow::Continue(())
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
}

/// 从用户任务的内核态返回到用户态。
///
/// 注意：会切换控制流和栈
pub fn trap_return(trap_context: *mut TrapContext) {
    set_user_trap_entry();
    extern "C" {
        fn __return_to_user(cx: *mut TrapContext);
    }
    // 对内核来说，调用 __return_to_user 返回内核态就好像一次函数调用
    // 因此编译器会将 Caller Saved 的寄存器保存下来
    // 但是 Called Saved 的寄存器很快会被覆盖，因此需要在 TrapContext 上保存下来
    unsafe { __return_to_user(trap_context) }
}

#[allow(dead_code)]
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

fn set_user_trap_entry() {
    extern "C" {
        fn __trap_from_user();
    }

    unsafe {
        // stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
        stvec::write(__trap_from_user as usize, TrapMode::Direct);
    }
}

fn set_kernel_trap_entry() {
    extern "C" fn trap_from_kernel() -> ! {
        panic!(
            "Trap from kernel! Cause = {:?}, bad addr = {:#x}, bad instruction = {:#x}",
            scause::read().cause(),
            stval::read(),
            sepc::read(),
        );
    }
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}
