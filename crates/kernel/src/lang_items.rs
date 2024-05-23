use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{tracer, uart_console::println};

static PANICKED: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    if let Ok(false) = PANICKED.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    {
        println!("{info}");
        tracer::print_span_stack();
    }
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::SystemFailure);
    unreachable!();
}
