use std::{
    error::Error,
    io::{BufRead, BufReader, Write},
    ops::Range,
    sync::LazyLock,
};

use clap::Parser;
use regex::Regex;

use crate::{build::BuildArgs, qemu::QemuArgs, tool};

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
            writeln!(stdin, "testrunner")?;

            let mut output = Vec::new();

            loop {
                let line = lines.next().unwrap().unwrap();
                if line.contains("==ALL TESTS OK==") {
                    break;
                }
                println!("{line}");
                output.push(line);
            }
            writeln!(stdin, "exit")?;

            let mut passed = Vec::new();
            let mut failed = Vec::new();

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
            for part in failed {
                if part.is_ptest {
                    println!("    ptest {}", part.name);
                } else {
                    println!("    ktest {}", part.name);
                }
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
