#![no_std]
#![no_main]

use user::{bench_main, exit, fork, println, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    bench_main(
        "bench_fork",
        || {
            let pid = fork();
            match pid {
                ..0 => println!("ERROR fork: {}", pid),
                0 => exit(0),
                _ => {
                    let mut exit_code = 0;
                    let ret = waitpid(pid as usize, &mut exit_code);
                    if ret < 0 {
                        println!("ERROR waitpid: {}", ret);
                    }
                }
            }
        },
        128,
    );
    0
}
