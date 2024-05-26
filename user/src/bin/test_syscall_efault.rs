#![no_std]
#![no_main]
#![feature(strict_provenance)]

use defines::error::errno;
use user::{sys_execve, sys_uname, sys_write, test_main, STDOUT};

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_syscall_efault", || {
        // 测试 `check_cstr()`
        let ret = unsafe { sys_execve(core::ptr::dangling(), core::ptr::dangling()) };
        assert_eq!(ret, errno::EFAULT.as_isize());

        // 测试 `check_slice()`
        let ret = sys_write(STDOUT, core::ptr::dangling(), 4);
        assert_eq!(ret, errno::EFAULT.as_isize());

        // 测试 `check_ptr_mut()`
        let ret = unsafe { sys_uname(core::ptr::dangling_mut()) };
        assert_eq!(ret, errno::EFAULT.as_isize());
    });
    0
}
