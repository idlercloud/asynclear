#![no_std]
#![no_main]

use user::{bench_main, chdir, println, sys_getcwd};

#[no_mangle]
pub fn main() -> i32 {
    let e = chdir(c"/ktest");
    if e < 0 {
        println!("ERROR chdir: {}", e);
        return -1;
    }
    let mut buf = [0; 128];
    bench_main(
        "bench_getcwd",
        || {
            for _ in 0..256 {
                let ret = sys_getcwd(&mut buf);
                if ret < 0 {
                    println!("getcwd error: {}", ret);
                }
            }
        },
        128,
    );
    0
}
