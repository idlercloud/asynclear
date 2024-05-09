#![no_std]
#![no_main]
#![allow(elided_lifetimes_in_paths)]
#![feature(strict_provenance)]
#![feature(format_args_nl)]
#![feature(const_binary_heap_constructor)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(step_trait)]
#![feature(ptr_metadata)]
#![feature(type_alias_impl_trait)]
#![feature(int_roundings)]
#![feature(array_chunks, iter_array_chunks)]
#![feature(coroutines, iter_from_coroutine)]
#![feature(maybe_uninit_uninit_array, maybe_uninit_array_assume_init)]
#![feature(sync_unsafe_cell)]

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
mod time;
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
