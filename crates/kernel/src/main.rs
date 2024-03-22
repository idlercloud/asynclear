#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(strict_provenance)]
#![feature(generic_const_exprs)]
#![feature(panic_info_message)]
#![feature(inline_const)]
#![feature(format_args_nl)]
#![feature(const_binary_heap_constructor)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(assert_matches)]
#![feature(step_trait)]

#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

mod drivers;
mod executor;
mod fs;
mod hart;
mod lang_items;
mod memory;
mod process;
mod signal;
mod syscall;
mod thread;
mod tracer;
mod trap;
mod uart_console;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub fn kernel_loop() -> ! {
    info!("Enter kernel loop");
    executor::run_utils_idle(|| SHUTDOWN.load(Ordering::SeqCst));

    info!("Exit kernel loop");
    let _guard = riscv_guard::NoIrqGuard::new();
    #[cfg(feature = "profiling")]
    tracer::report_profiling();
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}
