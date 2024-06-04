#![no_std]
#![no_main]

use user::{bench_main, println, sys_dup};

#[no_mangle]
pub fn main() -> i32 {
    bench_main(
        "bench_dup",
        || {
            for _ in 0..32 {
                let ret = sys_dup(1);
                if ret < 0 {
                    println!("ERROR dup: {}", ret);
                }
            }
        },
        16,
    );
    0
}
