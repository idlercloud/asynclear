#![no_std]
#![no_main]

use user::{println, sys_brk, test_main};

#[no_mangle]
fn main() -> i32 {
    test_main("test_brk", || {
        let orig = sys_brk(0) as usize;
        println!("heap pos: {}", orig);

        let mut brk = orig;
        for _ in 0..3 {
            brk += 64;
            assert_eq!(sys_brk(brk) as usize, brk);
            println!("heap pos: {}", brk);
        }

        assert_eq!(sys_brk(orig) as usize, orig);
    });
    0
}
