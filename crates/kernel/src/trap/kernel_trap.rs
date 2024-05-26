use riscv::register::{
    scause,
    scause::{Interrupt, Trap},
    sepc, stval,
    stvec::{self, TrapMode},
};

use crate::time;

pub fn set_kernel_trap_entry() {
    extern "C" {
        fn __trap_from_kernel();
    }
    unsafe {
        stvec::write(__trap_from_kernel as usize, TrapMode::Direct);
    }
}

/// Kernel trap handler
#[no_mangle]
pub extern "C" fn kernel_trap_handler() {
    match scause::read().cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            let _enter = debug_span!("timer_irq").entered();
            // TODO: 想办法通知线程让出 hart
            time::check_timer();
            riscv_time::set_next_trigger();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            let _enter = debug_span!("external_irq").entered();
            super::interrupt_handler();
        }
        other => {
            panic!(
                "Trap from kernel! Cause = {:?}, bad addr = {:#x}, bad instruction = {:#x}",
                other,
                stval::read(),
                sepc::read(),
            );
        }
    }
}
