#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(iter_intersperse)]

#[macro_use]
extern crate kernel_tracer;
extern crate alloc;

mod drivers;
mod glue;
mod lang_items;
mod syscall;
mod tracer;

use core::{
    arch,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use common::config::{HART_START_ADDR, MAX_HART_NUM};
use console_output::println;
use fdt::Fdt;
use libkernel::{extern_symbols, hart, memory, process};
use riscv::register::sstatus::{self, FS};

arch::global_asm!(include_str!("entry.S"));
arch::global_asm!(include_str!("trap.S"));

pub static BOOT_HART: AtomicUsize = AtomicUsize::new(usize::MAX);

#[unsafe(no_mangle)]
pub extern "C" fn __hart_entry(hart_id: usize, fdt_ptr: usize) -> ! {
    static INIT_FINISHED: AtomicBool = AtomicBool::new(false);

    // 主核启动
    if BOOT_HART
        .compare_exchange(usize::MAX, hart_id, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        clear_bss();
        unsafe {
            hart::set_local_hart(hart_id);
            memory::init();
        }
        memory::KERNEL_SPACE.activate();
        let fdt_ptr = fdt_ptr + common::config::PA_TO_VA;
        let fdt_ptr = fdt_ptr as *const u8;
        let fdt = unsafe { Fdt::from_ptr(fdt_ptr).unwrap() };
        // drivers 依赖于 mmio 映射（其实也许可以放在 boot page table 里？）
        drivers::init(&fdt);
        // log 实现依赖于 uart 和 virtio_block
        crate::tracer::init();
        enable_float();
        memory::log_kernel_sections();

        glue::init_vfs();

        executor::block_on(process::init());
        glue::spawn_user_thread(
            process::PROCESS_MANAGER
                .init_proc()
                .lock_inner_with(|inner| inner.main_thread()),
        );
        info!("Init hart {hart_id} started");
        INIT_FINISHED.store(true, Ordering::SeqCst);

        // 将下面的代码取消注释即可启动多核
        for i in 0..MAX_HART_NUM {
            if i == hart_id {
                continue;
            }
            sbi_rt::hart_start(i, HART_START_ADDR, 0);
        }
    } else {
        while !INIT_FINISHED.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
        unsafe {
            hart::set_local_hart(hart_id);
        }
        memory::KERNEL_SPACE.activate();
        enable_float();
        info!("Hart {hart_id} started");
    }

    let _enter = info_span!("hart", id = hart_id).entered();

    glue::init_trap();

    crate::kernel_loop();
}

pub fn kernel_loop() -> ! {
    info!("Enter kernel loop");
    executor::run_until_shutdown(|| {
        sbi_rt::hart_suspend(sbi_rt::Retentive, 0, 0);
    });

    info!("Exit kernel loop");
    let _guard = riscv_guard::NoIrqGuard::new();
    #[cfg(feature = "profiling")]
    tracer::report_profiling();
    let hart_id = hart::local_hart().hart_id();
    if hart_id != BOOT_HART.load(Ordering::SeqCst) {
        println!("hart {hart_id} wait boot hart to shutdown");
        loop {
            core::hint::spin_loop();
        }
    }
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}

fn clear_bss() {
    use extern_symbols::{ebss, sbss};
    let len = ebss as *const () as usize - sbss as *const () as usize;
    // 为 debug 模式做的优化，减少启动时间
    #[cfg(debug_assertions)]
    {
        // 似乎 `BATCH_SIZE` 为 4096 时效果最好
        // 是因为恰好为一个 `PAGE_SIZE` 吗
        const BATCH_SIZE: usize = 4096;
        let batch_end = sbss as *const () as usize + len / BATCH_SIZE * BATCH_SIZE;
        unsafe {
            core::slice::from_raw_parts_mut(sbss as *const () as usize as *mut [u8; BATCH_SIZE], len / BATCH_SIZE)
                .fill([0; BATCH_SIZE]);
            core::slice::from_raw_parts_mut(batch_end as *mut u8, ebss as *const () as usize - batch_end).fill(0);
        }
    }
    #[cfg(not(debug_assertions))]
    unsafe {
        core::slice::from_raw_parts_mut(sbss as *mut u8, len).fill(0);
    }
}

pub const DEFAULT_FCSR: u32 = {
    // exception when NV(invalid operation)
    let fflags: u32 = 0b10000;
    let round_mode: u32 = 0;
    (round_mode << 4) | fflags
};

fn enable_float() {
    unsafe {
        sstatus::set_fs(FS::Clean);
        arch::asm!("csrw fcsr, {}", in(reg) DEFAULT_FCSR);
        // 修改 `fcsr` 需要不为 `FS::Off`，且也会导致 `FS::Dirty`
        sstatus::set_fs(FS::Clean);
    }
}
