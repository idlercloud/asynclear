#![no_std]
#![no_main]

use core::hint::black_box;

use user::{println, test_main};

fn recursive(depth: usize, upper_bound: usize) -> usize {
    if black_box(depth >= upper_bound) {
        return depth;
    }
    let mut ret: usize = 512;
    let r = recursive(depth + 1, upper_bound);
    if r <= 1024 {
        ret = r;
    }
    ret
}

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_pid", || {
        let ret = recursive(1, 512);
        println!("{}", ret);
    });
    0
}
