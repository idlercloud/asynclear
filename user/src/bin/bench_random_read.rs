#![no_std]
#![no_main]

use defines::fs::{OpenFlags, SEEK_SET};
use fastrand::Rng;
use user::{bench_main, lseek, open, println, read};

#[no_mangle]
pub fn main() -> i32 {
    let fd = open(c"_playground", OpenFlags::RDONLY) as i32;
    if fd < 0 {
        println!("open file for read failed, {}", fd);
        return -1;
    }
    let mut rng = Rng::with_seed(19260817);
    let mut buf = [0; 4096];
    bench_main(
        "bench_random_read",
        || {
            for _ in 0..64 {
                let offset = rng.i64(0..1024 * 1024);
                if lseek(fd, offset, SEEK_SET) < 0 {
                    println!("seek failed");
                }
                if read(fd, &mut buf) < 0 {
                    println!("read failed");
                }
            }
        },
        64,
    );
    0
}
