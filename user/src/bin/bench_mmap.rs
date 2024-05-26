#![no_std]
#![no_main]

use defines::misc::{MmapFlags, MmapProt};
use user::{bench_main, println, sys_mmap, sys_munmap};

#[no_mangle]
pub fn main() -> i32 {
    bench_main(
        "bench_mmap",
        || {
            const TIMES: usize = 128;
            const LEN: usize = 5444;
            let mut starts = [0; TIMES];
            for start in &mut starts {
                let ret = sys_mmap(
                    0,
                    LEN,
                    MmapProt::PROT_READ,
                    MmapFlags::MAP_PRIVATE | MmapFlags::MAP_ANONYMOUS,
                    usize::MAX,
                    0,
                );
                if ret < 0 {
                    println!("ERROR mmap: {}", ret);
                    return;
                }
                *start = ret as usize;
            }
            for start in starts {
                let ret = sys_munmap(start, LEN);
                if ret < 0 {
                    println!("ERROR munmap: {}", ret);
                }
            }
        },
        32,
    );
    0
}
