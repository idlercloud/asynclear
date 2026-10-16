#![no_main]
#![no_std]

use core::ffi::CStr;

use defines::{
    error::errno::ENOENT,
    fs::OpenFlags,
    ktest::{KTEST_FAILED, KTEST_INCOMPLETE, KTEST_OK, KTEST_RUNNER_ERROR, TEST_EXIT_EXEC_ERROR, TEST_EXIT_MISSING},
    wait::{wexitstatus, wifexited, wtermsig},
};
use user::{chdir, close, exec, exit, fork, open, println, waitpid, write};

include!(concat!(env!("OUT_DIR"), "/ktest_list.rs"));

const PRELIMINARY_TESTS: [&CStr; 30] = [
    c"brk",
    c"chdir",
    c"clone",
    c"close",
    c"dup",
    c"dup2",
    c"execve",
    c"exit",
    c"fork",
    c"fstat",
    c"getcwd",
    c"getdents",
    c"getpid",
    c"getppid",
    c"gettimeofday",
    c"mmap",
    c"mount",
    c"munmap",
    c"open",
    c"pipe",
    c"read",
    c"sleep",
    c"times",
    c"umount",
    c"uname",
    c"unlink",
    c"wait",
    c"waitpid",
    c"write",
    c"yield",
];

/// 结果上报通道：往它写一个字节，内核就以该值结束整个模拟。只有 ktest 模式的镜像里有这个节点
const KTEST_DEVICE: &CStr = c"/dev/ktest";

#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut summary = Summary::default();

    chdir(c"/ptest");
    for test in PRELIMINARY_TESTS {
        summary.run(test);
    }
    println!("==PTEST DONE==");

    chdir(c"/ktest");
    for test in KTESTS {
        summary.run(test);
    }
    println!("==KTEST DONE==");

    println!(
        "==KTEST RESULT expected={} ran={} passed={} failed={} missing={}==",
        summary.expected, summary.ran, summary.passed, summary.failed, summary.missing
    );

    report(summary.result())
}

/// 逐测试结果与整轮计数
#[derive(Default)]
struct Summary {
    expected: usize,
    ran: usize,
    passed: usize,
    failed: usize,
    missing: usize,
}

impl Summary {
    /// fork 一个测试，并按它真实的退出码判定
    fn run(&mut self, test: &CStr) {
        self.expected += 1;

        let pid = fork();
        if pid < 0 {
            self.missing += 1;
            println!("==TEST {} MISSING (fork exit code: {})==", cstr_to_str(test), pid);
            return;
        }
        if pid == 0 {
            let ret = if test == c"test_echo" {
                exec(
                    test,
                    &[test.as_ptr().cast(), c"echo_example".as_ptr().cast(), core::ptr::null()],
                )
            } else {
                exec(test, &[test.as_ptr().cast(), core::ptr::null()])
            };
            // exec 在正常情况下不应返回
            let missing = ret == ENOENT.as_isize();
            println!(
                "==TEST {} {} (exec: {})==",
                cstr_to_str(test),
                if missing { "MISSING" } else { "ERROR" },
                ret
            );
            exit(if missing {
                TEST_EXIT_MISSING
            } else {
                TEST_EXIT_EXEC_ERROR
            });
        }

        let mut status = 0;
        waitpid(pid as usize, &mut status);
        let exit_code = wexitstatus(status);

        if wifexited(status) && (exit_code == TEST_EXIT_MISSING as u8 || exit_code == TEST_EXIT_EXEC_ERROR as u8) {
            self.missing += 1;
            return;
        }

        self.ran += 1;
        // 正常退出（且码为 0）才算通过；被信号杀死同样算异常
        // TODO:（内核对故障目前是用退出码编码的，等它改成 Linux 语义后 `wifsignaled` 才会在这里起作用）
        let abnormal = !wifexited(status) || exit_code != 0;
        // `test_should_fail_*` 的通过条件正好相反：必须异常退出
        let expected_abnormal = cstr_to_str(test).starts_with("test_should_fail_");
        // 打印在结论行里的标签
        let label = if abnormal == expected_abnormal {
            self.passed += 1;
            "PASS"
        } else {
            self.failed += 1;
            "FAIL"
        };
        if wifexited(status) {
            println!("==TEST {} {} (exit={})==", cstr_to_str(test), label, exit_code);
        } else {
            println!(
                "==TEST {} {} (signal={})==",
                cstr_to_str(test),
                label,
                wtermsig(status)
            );
        }
    }

    /// 整轮结果码，取值见 `defines::ktest`
    fn result(&self) -> u8 {
        if self.failed > 0 {
            KTEST_FAILED
        } else if self.missing > 0 {
            KTEST_INCOMPLETE
        } else {
            KTEST_OK
        }
    }
}

/// 把整轮结果提交给 `/dev/ktest` 继而退出内核
fn report(result: u8) -> ! {
    let fd = open(KTEST_DEVICE, OpenFlags::WRONLY);
    if fd < 0 {
        println!("==KTEST RUNNER ERROR: cannot open /dev/ktest (ret={})==", fd);
        exit(KTEST_RUNNER_ERROR as i32);
    }
    write(fd as i32, &[result]);
    close(fd as i32);
    // 写入成功的话上面那次 syscall 不会返回
    println!(
        "==KTEST RUNNER ERROR: writing {:#x} to /dev/ktest did not end the run==",
        result
    );
    exit(KTEST_RUNNER_ERROR as i32)
}

fn cstr_to_str(cstr: &CStr) -> &str {
    cstr.to_str().unwrap_or("?")
}
