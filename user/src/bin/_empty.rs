#![no_std]
#![no_main]

use user::exit;

#[no_mangle]
pub fn main() -> i32 {
    exit(0);
}
