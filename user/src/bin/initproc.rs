#![no_main]
#![no_std]

#[macro_use]
extern crate user;

use defines::wait::wexitstatus;
use user::{exec, fork, wait};

#[unsafe(no_mangle)]
fn main() -> i32 {
    if fork() == 0 {
        exec(c"shell", &[c"shell".as_ptr().cast(), core::ptr::null()]);
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            // No child
            if pid == -10 {
                println!("[initproc] No child process. OS shutdown");
                return 0;
            }
            if pid < 0 {
                panic!("Error with {pid}");
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid,
                wexitstatus(exit_code),
            );
        }
    }
    0
}
