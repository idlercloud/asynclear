#![no_std]
#![no_main]

use defines::fs::{OpenFlags, Stat};
use user::{bench_main, open, println, sys_newfstat};

#[no_mangle]
pub fn main() -> i32 {
    let fd = open(c"_playground", OpenFlags::RDONLY);
    if fd < 0 {
        println!("open file for read failed, {}", fd);
        return -1;
    }
    bench_main(
        "bench_fstat",
        || {
            let mut fstat = Stat::default();
            for _ in 0..256 {
                sys_newfstat(fd as usize, &mut fstat);
            }
        },
        128,
    );
    0
}
