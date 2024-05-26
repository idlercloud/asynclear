#![no_std]
#![no_main]

use defines::fs::OpenFlags;
use user::{bench_main, open, println, sys_getdents64};

#[no_mangle]
pub fn main() -> i32 {
    let mut buf = [0; 4096];
    bench_main(
        "bench_getdents",
        || {
            let fd = open(c"/", OpenFlags::RDONLY | OpenFlags::DIRECTORY);
            if fd < 0 {
                println!("open root dir failed: {}", fd);
            }
            let ret = sys_getdents64(fd as usize, &mut buf);
            if ret < 0 {
                println!("getdents failed: {}", ret);
            }
        },
        256,
    );
    0
}
