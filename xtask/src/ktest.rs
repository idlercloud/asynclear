use std::{
    io::{BufRead, BufReader, Write},
    ops::Range,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
    thread,
    time::Duration,
};

use clap::Parser;
use regex::Regex;

use crate::{build::BuildArgs, qemu::QemuArgs, tool};

/// 有测试没通过
const EXIT_TESTS_FAILED: u8 = 1;
/// 测试没跑完或结果不可信（超时、QEMU 异常退出、输出中断等）
const EXIT_HARNESS_ERROR: u8 = 2;

/// 运行内核集成测试
#[derive(Parser)]
pub struct KtestArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Hart 数量（SMP 代表 Symmetrical Multiple Processor）.
    #[clap(long, default_value_t = 2)]
    smp: u8,
    #[clap(long)]
    skip_build: bool,
    /// 如果开启，QEMU 会阻塞并等待 GDB 连接
    #[clap(long)]
    debug: bool,
    /// 整轮测试的超时时间（秒），超时后杀掉 QEMU 并以失败结束
    #[clap(long, default_value_t = 300)]
    timeout: u64,
}

impl KtestArgs {
    pub fn run_test(self) -> ExitCode {
        if !self.skip_build {
            self.build.build();
            tool::prepare_os();
        }

        println!("Running qemu...");

        let mut spawned = QemuArgs::base_qemu()
            .args(["-smp", &self.smp.to_string()])
            .optional_args(self.debug.then_some(["-s", "-S"]))
            .spawn();
        let mut stdin = spawned.stdin.take().unwrap();
        let stdout = spawned.stdout.take().unwrap();
        let child = Arc::new(Mutex::new(spawned));
        // 提前返回或 panic 时也不能把仍在运行的 QEMU 留在后台占用文件镜像
        let _guard = scopeguard::guard(Arc::clone(&child), |child| {
            let mut child = child.lock().unwrap();
            if child.try_wait().is_ok_and(|status| status.is_none()) {
                let _ = child.kill();
                let _ = child.wait();
            }
        });
        let timed_out = Arc::new(AtomicBool::new(false));

        {
            // 挂死的测试只能由宿主机兜底：读循环会一直阻塞，QEMU 也不会自己退出
            let child = Arc::clone(&child);
            let timed_out = Arc::clone(&timed_out);
            let timeout = Duration::from_secs(self.timeout);
            thread::spawn(move || {
                thread::sleep(timeout);
                let mut child = child.lock().unwrap();
                if child.try_wait().is_ok_and(|status| status.is_none()) {
                    timed_out.store(true, Ordering::SeqCst);
                    let _ = child.kill();
                }
            });
        }

        let mut lines = BufReader::new(stdout).lines();
        let output = (|| -> anyhow::Result<Vec<String>> {
            // 等待 shell 准备完毕
            loop {
                let Some(line) = lines.next() else {
                    anyhow::bail!("QEMU stopped before the shell was ready");
                };
                if line?.contains("Rust user shell") {
                    break;
                }
            }

            // 开始运行测试
            writeln!(stdin, "testrunner")?;
            let mut output = Vec::new();

            loop {
                let Some(line) = lines.next() else {
                    anyhow::bail!("QEMU stopped before the end marker");
                };
                let line = line?;
                // 按照约定，testrunner 运行完后会输出这个，作为测试结束的标志
                if line.contains("==ALL TESTS OK==") {
                    break;
                }
                println!("{line}");
                output.push(line);
            }
            writeln!(stdin, "exit")?;
            Ok(output)
        })();

        let output = match output {
            // 超时会打断读循环，此时优先报告超时而不是读取错误
            Err(_) if timed_out.load(Ordering::SeqCst) => {
                return report_harness_error(format!("timed out after {}s", self.timeout));
            }
            Err(e) => return report_harness_error(format!("failed to read QEMU output: {e}")),
            Ok(output) => output,
        };

        let mut passed = Vec::new();
        let mut failed = Vec::new();

        // 根据文本内容解析所有测试输出
        let ptest_re = Regex::new("========== START (test_.+) ==========").unwrap();
        let ktest_re = Regex::new("----(test_.+) begins----").unwrap();
        let mut parts = Vec::new();
        #[derive(Clone, Debug)]
        struct OutputPart<'a> {
            name: &'a str,
            is_ptest: bool,
            range: Range<usize>,
        }
        let mut curr_part = OutputPart {
            name: "",
            is_ptest: true,
            range: 0..0,
        };
        let mut in_part = false;

        for (i, line) in output.iter().enumerate() {
            if let Some(caps) = ptest_re.captures(line) {
                if in_part {
                    parts.push(curr_part.clone());
                    curr_part.is_ptest = true;
                }
                curr_part.name = caps.get(1).unwrap().as_str();
                curr_part.range = i..i + 1;
                in_part = true;
            } else if let Some(caps) = ktest_re.captures(line) {
                if in_part {
                    parts.push(curr_part.clone());
                    curr_part.is_ptest = false;
                }
                curr_part.name = caps.get(1).unwrap().as_str();
                curr_part.range = i..i + 1;
                in_part = true;
            } else if in_part {
                curr_part.range.end = i + 1;
            }
        }

        if in_part {
            parts.push(curr_part);
        }

        for part in parts {
            if (part.is_ptest && ptest_checker(part.name, &output[part.range.clone()]))
                || (!part.is_ptest && ktest_checker(part.name, &output[part.range.clone()]))
            {
                passed.push(part);
            } else {
                failed.push(part);
            }
        }

        println!("Passed tests:");
        for part in passed {
            if part.is_ptest {
                println!("    ptest {}", part.name);
            } else {
                println!("    ktest {}", part.name);
            }
        }
        println!("Failed tests:");
        for part in &failed {
            if part.is_ptest {
                println!("    ptest {}", part.name);
            } else {
                println!("    ktest {}", part.name);
            }
        }

        let mut shutdown = false;
        for line in lines {
            if line.is_ok_and(|line| line.contains("[initproc] No child process. OS shutdown")) {
                shutdown = true;
                break;
            }
        }

        let wait_result = {
            let mut child = child.lock().unwrap();
            child.wait()
        };
        let status = match wait_result {
            Ok(status) => status,
            Err(e) => return report_harness_error(format!("failed to wait for QEMU: {e}")),
        };

        if timed_out.load(Ordering::SeqCst) {
            return report_harness_error(format!("timed out after {}s", self.timeout));
        }
        if !shutdown {
            return report_harness_error("QEMU shutdown marker not found");
        }
        if !status.success() {
            return report_harness_error(format!("QEMU exited with {status}"));
        }
        if !failed.is_empty() {
            println!("{} test(s) failed", failed.len());
            return ExitCode::from(EXIT_TESTS_FAILED);
        }
        ExitCode::SUCCESS
    }
}

/// 打印测试没有跑完的原因，QEMU 由作用域守卫在返回时回收
fn report_harness_error(message: impl std::fmt::Display) -> ExitCode {
    println!("ERROR: {message}");
    ExitCode::from(EXIT_HARNESS_ERROR)
}

fn ptest_checker(name: &str, content: &[String]) -> bool {
    if name == "test_execve" {
        if !content
            .iter()
            .any(|line| line.contains("========== END main =========="))
        {
            return false;
        }
    } else if !content
        .iter()
        .any(|line| line.contains(&format!("========== END {name} ==========")))
    {
        return false;
    }
    if name == "test_brk" {
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"heap pos: (\d+)").unwrap());
        let content = content.join("\n");
        let heaps = RE
            .captures_iter(&content)
            .map(|c| c[1].parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        if heaps.len() != 3 {
            return false;
        }
        if heaps[0] + 64 != heaps[1] || heaps[1] + 64 != heaps[2] {
            return false;
        }
    }
    // TODO: 写其它测试的 checker
    true
}

fn ktest_checker(name: &str, content: &[String]) -> bool {
    let should_fail = name.contains("test_should_fail_");
    let contain_end = content
        .iter()
        .any(|line| line.contains(&format!("----{name} ends  ----")));
    if (contain_end && should_fail) || (!contain_end && !should_fail) {
        return false;
    }
    if name == "test_echo" {
        return content[1] == "echo_example";
    }
    // TODO: 写其它测试的 checker
    true
}
