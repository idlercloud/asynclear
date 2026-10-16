#![no_std]
#![no_main]

use user::{println, test_main};

#[no_mangle]
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    test_main("test_echo", || {
        assert_eq!(argc, 2);
        assert_eq!(argv[1], "echo_example");
        println!("{}", argv[1]);
    });
    0
}
