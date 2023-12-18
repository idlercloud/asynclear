#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(inline_const)]
#![feature(format_args_nl)]
#![feature(const_binary_heap_constructor)]

#[macro_use]
extern crate uart_console;
#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

use crate::process::INITPROC;

mod hart;
mod lang_items;
mod log_impl;
mod process;
mod syscall;
mod thread;
mod trap;

pub fn kernel_loop() -> ! {
    info!("Enter kernel loop");
    thread::spawn_user_thread(INITPROC.lock_inner(|inner| inner.main_thread()));
    executor::run_utils_idle();

    info!("Exit kernel loop");

    #[cfg(feature = "profiling")]
    log_impl::report_profiling();
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}
