#![no_std]
#![no_main]

use user::{exit, fork, getpid, getppid, println, test_main, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_pid", || {
        let parent_pid = getpid();
        let child_pid = fork();
        if child_pid == 0 {
            assert_eq!(getppid(), parent_pid);
            exit(0);
        } else {
            println!("parent pid: {}", parent_pid);
            println!("child pid: {}", child_pid);
            let mut exit_code = 0;
            let exit_pid = waitpid(child_pid as usize, &mut exit_code);
            assert_eq!(child_pid, exit_pid);
            if exit_code != 0 {
                panic!("exit with error code {exit_code}");
            }
        }
    });
    0
}
