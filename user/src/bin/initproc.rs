#![no_main]
#![no_std]

#[macro_use]
extern crate user;

use user::{exec, fork, wait};

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        exec("shell\0", &["shell\0".as_ptr(), core::ptr::null()]);
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
                panic!("Error with {}", pid);
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid,
                exit_code >> 8,
            );
        }
    }
    0
}
