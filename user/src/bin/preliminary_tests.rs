#![no_main]
#![no_std]

use user::{exec, exit, fork, wait};

const PRELIMINARY_TESTS: [&str; 32] = [
    "brk\0",
    "chdir\0",
    "clone\0",
    "close\0",
    "dup\0",
    "dup2\0",
    "execve\0",
    "exit\0",
    "fork\0",
    "fstat\0",
    "getcwd\0",
    "getdents\0",
    "getpid\0",
    "getppid\0",
    "gettimeofday\0",
    "mkdir_\0",
    "mmap\0",
    "mount\0",
    "munmap\0",
    "open\0",
    "openat\0",
    "pipe\0",
    "read\0",
    "sleep\0",
    "times\0",
    "umount\0",
    "uname\0",
    "unlink\0",
    "wait\0",
    "waitpid\0",
    "write\0",
    "yield\0",
];

#[no_mangle]
fn main() -> i32 {
    for test in PRELIMINARY_TESTS {
        if fork() == 0 {
            if exec(test, &[test.as_ptr(), core::ptr::null()]) < 0 {
                exit(-1);
            }
        } else {
            let mut exit_code = 0;
            wait(&mut exit_code);
        }
    }
    0
}
