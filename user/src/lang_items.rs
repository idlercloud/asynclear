use crate::exit;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo<'_>) -> ! {
    let err = panic_info.message();
    if let Some(location) = panic_info.location() {
        println!("Panicked at {}:{}, {}", location.file(), location.line(), err);
    } else {
        println!("Panicked: {}", err);
    }
    exit(-1);
}
