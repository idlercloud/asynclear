use std::{
    error::Error,
    io::{BufRead, BufReader, Write},
};

use clap::Parser;

use crate::{build::BuildArgs, qemu::QemuArgs, tool};

/// 运行内核集成测试
#[derive(Parser)]
pub struct KtestArgs {
    /// Hart 数量（SMP 代表 Symmetrical Multiple Processor）.
    #[clap(long, default_value_t = 2)]
    smp: u8,
    /// 测试运行次数
    #[clap(short, long, default_value_t = 1)]
    times: usize,
    /// 运行哪些测试，若为空则运行所有。暂时未实现
    test_names: Option<Vec<String>>,
}

impl KtestArgs {
    pub fn run_test(self) {
        BuildArgs::build_for_test();
        tool::prepare_os();

        println!("Running qemu...");

        let mut child = QemuArgs::base_qemu()
            .args(["-smp", &self.smp.to_string()])
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
            writeln!(stdin, "exit")?;
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
