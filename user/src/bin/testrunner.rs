#![no_main]
#![no_std]

use core::ffi::CStr;

use defines::error::errno::ENOENT;
use user::{chdir, exec, exit, fork, println, waitpid};

const PRELIMINARY_TESTS: [&CStr; 32] = [
    c"brk",
    c"chdir",
    c"clone",
    c"close",
    c"dup",
    c"dup2",
    c"execve",
    c"exit",
    c"fork",
    c"fstat",
    c"getcwd",
    c"getdents",
    c"getpid",
    c"getppid",
    c"gettimeofday",
    c"mkdir_",
    c"mmap",
    c"mount",
    c"munmap",
    c"open",
    c"openat",
    c"pipe",
    c"read",
    c"sleep",
    c"times",
    c"umount",
    c"uname",
    c"unlink",
    c"wait",
    c"waitpid",
    c"write",
    c"yield",
];

const KTESTS: [&CStr; 10] = [
    c"test_echo",
    c"test_fork",
    c"test_lazy_stack",
    c"test_pid",
    c"test_power",
    c"test_should_fail_bad_address",
    c"test_should_fail_bad_instructions",
    c"test_should_fail_bad_register",
    c"test_syscall_efault",
    c"test_yield",
];

#[no_mangle]
fn main() -> i32 {
    chdir(c"/ptest");
    for test in PRELIMINARY_TESTS {
        let pid = fork();
        if pid < 0 {
            println!("[testsrunner] ERROR fork: {}", pid);
            continue;
        }
        if pid == 0 {
            let ret = exec(test, &[test.as_ptr().cast(), core::ptr::null()]);
            if ret < 0 && ret != ENOENT.as_isize() {
                println!("[testrunner] ERROR exec: {}", ret);
                exit(-1);
            }
            exit(0);
        } else {
            let mut exit_code = 0;
            waitpid(pid as usize, &mut exit_code);
        }
    }
    println!("==ALL PTEST OK==");
    chdir(c"/ktest");
    for test in KTESTS {
        let pid = fork();
        if pid < 0 {
            println!("[testsrunner] ERROR fork: {}", pid);
            continue;
        }
        if pid == 0 {
            exec_test(test);
        } else {
            let mut exit_code = 0;
            waitpid(pid as usize, &mut exit_code);
        }
    }
    println!("==ALL KTEST OK==");

    println!("==ALL TESTS OK==");
    0
}

fn exec_test(test_name: &CStr) {
    if test_name == c"test_echo" {
        exec(
            test_name,
            &[
                test_name.as_ptr().cast(),
                c"echo_example".as_ptr().cast(),
                core::ptr::null(),
            ],
        );
    } else {
        exec(test_name, &[test_name.as_ptr().cast(), core::ptr::null()]);
    }
    unreachable!()
}
