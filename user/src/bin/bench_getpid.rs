#![no_std]
#![no_main]

use user::{bench_main, getpid};

#[no_mangle]
pub fn main() -> i32 {
    bench_main(
        "bench_getpid",
        || {
            for _ in 0..1024 {
                getpid();
            }
        },
        32,
    );
    0
}
