#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(inline_const)]
#![feature(format_args_nl)]
#![feature(const_binary_heap_constructor)]

#[macro_use]
extern crate sbi_console;
#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

use crate::{hart::local_hart, process::INITPROC};

mod hart;
mod lang_items;
mod process;
mod syscall;
mod thread;
mod trap;

pub fn kernel_loop() -> ! {
    {
        let hart_id = unsafe { (*local_hart()).hart_id() };
        let _enter = info_span!("hart", id = hart_id).entered();
        info!("Enter kernel loop");

        thread::spawn_user_thread(INITPROC.lock_inner(|inner| inner.main_thread()));
        executor::run_utils_idle();

        info!("Exit kernel loop");
    }
    kernel_tracer::report_profiling();
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}
