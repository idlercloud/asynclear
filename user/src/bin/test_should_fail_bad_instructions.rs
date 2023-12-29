#![no_std]
#![no_main]

use user::test_main;

#[no_mangle]
pub fn main() -> isize {
    test_main("test_should_fail_bad_instructions", || unsafe {
        core::arch::asm!("sret");
    });
    0
}
