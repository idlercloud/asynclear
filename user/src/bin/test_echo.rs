#![no_std]
#![no_main]

use user::{println, test_main};

#[no_mangle]
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    test_main("test_echo", || {
        assert!(argc == 2);
        println!("{}", argv[1]);
    });
    0
}
