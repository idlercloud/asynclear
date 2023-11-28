#![no_std]
#![no_main]

#[macro_use]
extern crate user;
extern crate alloc;

#[no_mangle]
pub fn main(argc: usize, argv: &[&str]) -> i32 {
    let mut i = 1;
    while i < argc {
        println!("{}", argv[i]);
        i += 1;
    }
    0
}
