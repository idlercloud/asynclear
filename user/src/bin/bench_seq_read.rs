#![no_std]
#![no_main]

use defines::fs::{OpenFlags, SEEK_SET};
use user::{bench_main, lseek, open, println, read};

#[no_mangle]
pub fn main() -> i32 {
    let fd = open(c"_playground", OpenFlags::RDONLY) as i32;
    if fd < 0 {
        println!("open file for read failed, {}", fd);
        return -1;
    }
    let mut buf = [0; 4096];
    bench_main(
        "bench_seq_read",
        || {
            if lseek(fd, 0, SEEK_SET) < 0 {
                println!("seek failed");
            }
            while read(fd, &mut buf) > 0 {}
        },
        16,
    );
    0
}
