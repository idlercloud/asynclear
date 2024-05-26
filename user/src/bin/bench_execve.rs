#![no_std]
#![no_main]

use user::{bench_main, exec, exit, fork, println, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    bench_main(
        "bench_execve",
        || {
            let pid = fork();
            if pid < 0 {
                println!("ERROR fork: {}", pid);
                return;
            }

            if pid == 0 {
                let ret = exec(c"_empty", &[c"_empty".as_ptr().cast(), core::ptr::null()]);
                if ret < 0 {
                    println!("ERROR exec: {}", ret);
                    exit(0);
                }
            } else {
                let mut exit_code = 0;
                let ret = waitpid(pid as usize, &mut exit_code);
                if ret < 0 {
                    println!("waitpid failed: {}", ret);
                }
            }
        },
        24,
    );
    0
}
