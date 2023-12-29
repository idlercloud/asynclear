#![no_std]
#![no_main]

use user::test_main;

/// 由于 rustsbi 的问题，该程序无法正确退出
/// > rustsbi 0.2.0-alpha.1 已经修复，可以正常退出

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
