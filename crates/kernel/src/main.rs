#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(inline_const)]
#![feature(format_args_nl)]

use crate::{hart::local_hart, process::INITPROC};

mod hart;
mod lang_items;
mod process;
mod syscall;
mod thread;
mod trap;

#[macro_use]
extern crate sbi_console;
#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

pub fn kernel_loop() -> ! {
    let hart_id = unsafe { (*local_hart()).hart_id() };
    info!("Hart {hart_id} enter kernel loop");

    thread::spawn_user_thread(INITPROC.lock_inner(|inner| inner.main_thread()));
    let completed_task_num = executor::run_utils_idle();

    info!("Hart {hart_id} executed {completed_task_num} tasks");
    info!("Hart {hart_id} exit kernel loop");
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}
