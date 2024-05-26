use std::{
    error::Error,
    io::{BufRead, BufReader, Lines, Write},
    iter,
    process::{ChildStdin, ChildStdout},
};

use clap::Parser;

use crate::{
    build::{BuildArgs, USER_BINS},
    qemu::QemuArgs,
    tool,
};

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
}

impl KtestArgs {
    pub fn run_test(self) {
        if !self.skip_build {
            self.build.build();
            tool::prepare_os();
        }

        println!("Running qemu...");

        let mut child = QemuArgs::base_qemu()
            .args(["-smp", &self.smp.to_string()])
            .optional_arg(self.debug.then_some("-s"))
            .optional_arg(self.debug.then_some("-S"))
            .spawn();
        let stdin = child.stdin.as_mut().unwrap();
        let mut lines = BufReader::new(child.stdout.as_mut().unwrap()).lines();
        // 等待 shell 准备完毕
        || -> Result<(), Box<dyn Error>> {
            loop {
                let line = lines.next().unwrap()?;
                if line.contains("Rust user shell") {
                    break;
                }
            }
            let mut passed = Vec::new();
            let mut failed = Vec::new();

            for test_name in USER_BINS.iter() {
                if !test_name.starts_with("test_") {
                    continue;
                }
                let ok = handle_ktest(&mut lines, test_name, stdin)?;
                if ok {
                    passed.push(test_name);
                } else {
                    failed.push(test_name);
                }
            }

            writeln!(stdin, "exit")?;

            println!("Passed tests:");
            for name in passed {
                println!("    {name}");
            }
            println!("Failed tests:");
            for name in failed {
                println!("    {name}");
            }
            Ok(())
        }()
        .unwrap();
        let mut shutdown = false;
        for line in lines {
            if line
                .unwrap()
                .contains("[initproc] No child process. OS shutdown")
            {
                shutdown = true;
                break;
            }
        }
        if !shutdown {
            panic!("ERROR: no shutdown");
        }
        child.wait().unwrap();
    }
}

fn handle_ktest(
    lines: &mut Lines<BufReader<&mut ChildStdout>>,
    test_name: &str,
    stdin: &mut ChildStdin,
) -> Result<bool, Box<dyn Error>> {
    let mut get_test_output = || -> String {
        // 清除多余的输出
        let mut output = String::new();

        let mut need_clean = true;
        loop {
            let line = lines.next().unwrap().unwrap();
            println!("{line}");
            output.push_str(&line);
            output.push('\n');
            if line.contains("ends  ----") {
                break;
            }
            if line.contains("exited with code") {
                need_clean = false;
                break;
            }
        }
        while need_clean {
            let line = lines.next().unwrap().unwrap();
            println!("[consume rest] {line}");
            if line.contains("exited with code") {
                need_clean = false;
            }
        }
        output
    };
    if test_name == "test_echo" {
        let echo_content = iter::repeat_with(fastrand::alphanumeric)
            .take(fastrand::usize(16..32))
            .collect::<String>();
        writeln!(stdin, "ktest/{test_name} {echo_content}")?;
        let output = get_test_output();
        Ok(output.contains(&echo_content) && output.contains("ends  ----"))
    } else {
        writeln!(stdin, "ktest/{test_name}")?;
        let output = get_test_output();
        // 这样可能可读性更高点
        if test_name.contains("_should_fail_") {
            Ok(!output.contains("ends  ----"))
        } else {
            Ok(output.contains("ends  ----"))
        }
    }
}
