use std::{fs, sync::LazyLock};

use clap::Parser;
use const_format::formatcp;
use tap::Tap;

use crate::{cmd_util::Cmd, variables::TARGET_ARCH};

#[derive(Parser)]
pub struct BuildArgs {
    /// 控制台日志级别
    #[clap(long)]
    clog: Option<String>,
    /// 文件日志级别
    #[clap(long, default_value_t = String::from("TRACE"))]
    flog: String,
}

impl BuildArgs {
    pub fn build(&self) {
        println!("Building user apps...");
        Cmd::cmd("cargo build --package user --release")
            .args(&["--target", TARGET_ARCH])
            .args(&["--target-dir", "user/target"])
            .env("RUSTFLAGS", "-Clink-arg=-Tuser/src/linker.ld")
            .invoke();

        println!("Building kernel...");
        Cmd::cmd("cargo build --package kernel --release")
            .args(&["--target", TARGET_ARCH])
            .env(
                "RUSTFLAGS",
                "-Clink-arg=-Tcrates/kernel/src/linker.ld -Cforce-unwind-tables=yes",
            )
            .tap_mut(|cmd| {
                if let Some(clog) = &self.clog {
                    cmd.env("KERNEL_CLOG", clog);
                }
                cmd.env("KERNEL_FLOG", &self.flog);
            })
            .invoke();
    }
}

pub static USER_BINS: LazyLock<Vec<String>> = LazyLock::new(|| {
    fs::read_dir(formatcp!("user/src/bin"))
        .expect("Cannot read user bin crates directory")
        .map(|entry| {
            entry
                .expect("Failed reading user bin crate")
                .file_name()
                .to_string_lossy()
                .trim_end_matches(".rs")
                .to_owned()
        })
        .collect::<Vec<_>>()
});
