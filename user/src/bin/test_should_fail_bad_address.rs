#![no_std]
#![no_main]

use user::test_main;

#[no_mangle]
pub fn main() -> isize {
    test_main("test_should_fail_bad_address", || unsafe {
        core::ptr::null_mut::<u8>().write_volatile(0);
    });
    0
}
