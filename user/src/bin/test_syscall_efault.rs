#![no_std]
#![no_main]

use defines::error::errno;
use user::{exec, sys_uname, test_main, write, STDOUT};

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_syscall_efault", || {
        let invalid = unsafe {
            let invalid = core::slice::from_raw_parts(core::ptr::null(), 1);
            core::str::from_utf8_unchecked(invalid)
        };
        // 测试 `check_cstr()`
        let ret = exec(invalid, &[invalid.as_ptr()]);
        assert_eq!(ret, errno::EFAULT.as_isize());

        // 测试 `check_slice()`
        let ret = write(STDOUT, invalid.as_bytes());
        assert_eq!(ret, errno::EFAULT.as_isize());

        // 测试 `check_ptr_mut()`
        let ret = sys_uname(0x40 as _);
        assert_eq!(ret, errno::EFAULT.as_isize());
    });
    0
}
