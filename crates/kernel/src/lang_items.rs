use core::panic::PanicInfo;

use atomic::Ordering;
use console_output::eprintln;
use libkernel::hart;
use riscv_guard::NoIrqGuard;

use crate::{tracer, BOOT_HART};

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let _guard = NoIrqGuard::new();
    let hart = hart::local_hart();
    if !hart.panicked.get() {
        hart.panicked.set(true);
        eprintln!("{info}");
        unsafe {
            tracer::print_span_stack();
        }
    }

    executor::SHUTDOWN.store(true, Ordering::SeqCst);

    if hart.hart_id() != BOOT_HART.load(Ordering::SeqCst) {
        eprintln!("hart {} wait boot hart to shutdown", hart.hart_id());
        loop {}
    }

    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::SystemFailure);
    unreachable!();
}
