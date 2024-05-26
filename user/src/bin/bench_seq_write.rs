#![no_std]
#![no_main]

use defines::fs::{OpenFlags, SEEK_SET};
use fastrand::Rng;
use user::{bench_main, lseek, open, println, write_all};

#[no_mangle]
pub fn main() -> i32 {
    let fd = open(c"_playground", OpenFlags::RDWR);
    if fd < 0 {
        println!("open file for read failed, {}", fd);
        return -1;
    }
    let fd = fd as usize;
    let mut rng = Rng::with_seed(19260817);
    let mut buf = [0; 4096];
    bench_main(
        "bench_seq_write",
        || {
            if lseek(fd, 0, SEEK_SET) < 0 {
                println!("seek failed");
            }
            let mut tot_write = 0;
            while tot_write < 1024 * 1024 {
                buf.fill_with(|| rng.u8(0..=255));
                let ret = write_all(fd, &buf);
                if ret < 0 {
                    println!("write failed");
                }
                tot_write += ret;
            }
        },
        16,
    );
    0
}
