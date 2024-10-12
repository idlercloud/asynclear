#![no_std]
#![no_main]

use user::{exit, fork, println, test_main, waitpid, yield_};

const WIDTH: usize = 8;
const HEIGHT: usize = 4;

#[no_mangle]
fn main() -> i32 {
    test_main("test_yield", || {
        let pid = fork();
        if pid == 0 {
            for i in 0..HEIGHT {
                let buf = [b'B'; WIDTH];
                println!("{} [{}/{}]", core::str::from_utf8(&buf).unwrap(), i + 1, HEIGHT);
                yield_();
            }
            exit(0);
        } else {
            for i in 0..HEIGHT {
                let buf = [b'A'; WIDTH];
                println!("{} [{}/{}]", core::str::from_utf8(&buf).unwrap(), i + 1, HEIGHT);
                yield_();
            }
            let mut exit_code = 0;
            let exit_pid = waitpid(pid as usize, &mut exit_code);
            assert_eq!(pid, exit_pid);
            if exit_code != 0 {
                panic!("exit with error code {exit_code}");
            }
        }
    });
    0
}
