#![no_std]
#![no_main]

use defines::error::errno;
use user::{exec, test_main};

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_syscall_efault", || {
        let invalid = unsafe {
            let invalid = core::slice::from_raw_parts(core::ptr::null(), 1);
            core::str::from_utf8_unchecked(invalid)
        };
        let ret = exec(invalid, &[invalid.as_ptr()]);
        assert_eq!(ret, errno::EFAULT.as_isize());
    });
    0
}
