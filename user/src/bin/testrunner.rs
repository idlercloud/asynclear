#![no_main]
#![no_std]

use core::ffi::CStr;

use defines::{
    error::errno::ENOENT,
    ktest::{TEST_EXIT_EXEC_ERROR, TEST_EXIT_MISSING},
};
use user::{chdir, exec, exit, fork, println, waitpid};

include!(concat!(env!("OUT_DIR"), "/ktest_list.rs"));

const PRELIMINARY_TESTS: [&CStr; 30] = [
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
    c"mmap",
    c"mount",
    c"munmap",
    c"open",
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

#[no_mangle]
fn main() -> i32 {
    chdir(c"/ptest");
    for test in PRELIMINARY_TESTS {
        let pid = fork();
        if pid < 0 {
            println!("==TEST {} MISSING (fork: {})==", cstr_to_str(test), pid);
            continue;
        }
        if pid == 0 {
            let ret = exec(test, &[test.as_ptr().cast(), core::ptr::null()]);
            if ret < 0 {
                let missing = ret == ENOENT.as_isize();
                println!(
                    "==TEST {} {} (exec: {})==",
                    cstr_to_str(test),
                    if missing { "MISSING" } else { "ERROR" },
                    ret
                );
                exit(if missing { TEST_EXIT_MISSING } else { TEST_EXIT_EXEC_ERROR });
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
            println!("==TEST {} MISSING (fork: {})==", cstr_to_str(test), pid);
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

fn cstr_to_str(cstr: &CStr) -> &str {
    cstr.to_str().unwrap_or("?")
}

fn exec_test(test_name: &CStr) -> ! {
    let ret = if test_name == c"test_echo" {
        exec(
            test_name,
            &[
                test_name.as_ptr().cast(),
                c"echo_example".as_ptr().cast(),
                core::ptr::null(),
            ],
        )
    } else {
        exec(test_name, &[test_name.as_ptr().cast(), core::ptr::null()])
    };
    let missing = ret == ENOENT.as_isize();
    println!(
        "==TEST {} {} (exec: {})==",
        cstr_to_str(test_name),
        if missing { "MISSING" } else { "ERROR" },
        ret
    );
    exit(if missing { TEST_EXIT_MISSING } else { TEST_EXIT_EXEC_ERROR })
}
