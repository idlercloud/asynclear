#![no_std]
#![no_main]

use core::hint::black_box;

use user::{
    check_syscall_ret, close, exit, fork, pipe, println, read, test_main, wait, write_all, yield_,
};

#[no_mangle]
pub fn main() -> i32 {
    test_main("test_float_ctx", || {
        // 用 `pipe()` 做同步
        let mut pipe_fds = [0, 0];
        check_syscall_ret("pipe", pipe(&mut pipe_fds)).unwrap();

        let pid = check_syscall_ret("fork", fork()).unwrap();
        if pid == 0 {
            // 关闭写端
            check_syscall_ret("close", close(pipe_fds[1])).unwrap();

            // 共 16 个负责计算的进程
            for i in 0..16 {
                let pid = check_syscall_ret("fork", fork()).unwrap();
                if pid == 0 {
                    let mut buf = [0];
                    check_syscall_ret("read", read(pipe_fds[0], &mut buf)).unwrap();

                    let mut floats: [f64; 32] = [4.0; 32];
                    for (j, float) in floats.iter_mut().enumerate() {
                        *float = black_box(i + 1) as f64 * black_box(j) as f64 * 4.0;
                    }
                    floats[2] = 8.4;
                    yield_();
                    let mut sum = 0.0;
                    for f in floats {
                        sum += black_box(f);
                    }
                    println!("child {} sum: {}", i, sum);
                    exit(0);
                }
            }

            for _ in 0..16 {
                let mut exit_code = 0;
                check_syscall_ret("wait", wait(&mut exit_code)).unwrap();
            }

            exit(0);
        } else {
            check_syscall_ret("close", close(pipe_fds[0])).unwrap();
            let f1: f32 = 1.0;
            let f2: f64 = 2.0;
            println!("before f1: {}, f2: {}", f1, f2);
            let buf = [1; 16];
            check_syscall_ret("write", write_all(pipe_fds[1], &buf)).unwrap();
            yield_();

            let mut exit_code = 0;
            check_syscall_ret("wait", wait(&mut exit_code)).unwrap();
            println!("after f1: {}, f2: {}", f1, f2);
            assert_eq!(f1, 1.0);
            assert_eq!(f2, 2.0);
        }
    });
    0
}
