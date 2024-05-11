#![no_std]
#![no_main]

use user::test_main;

#[no_mangle]
pub fn main() -> isize {
    test_main("test_should_fail_bad_register", || {
        let mut sstatus: usize;
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        }
        panic!("(should not) get sstatus {sstatus}");
    });
    0
}
