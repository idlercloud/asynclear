use std::{fs, sync::LazyLock};

use clap::Parser;
use tap::Tap;

use crate::{cmd_util::Cmd, variables::TARGET_ARCH};

/// 构建内核和用户程序
#[derive(Parser)]
pub struct BuildArgs {
    /// 是否以 release 构建内核
    #[clap(long)]
    release: bool,
    /// 是否开启 profiling
    #[clap(long)]
    profiling: bool,
    /// 控制台日志级别
    #[clap(long, default_value_t = String::from("INFO"))]
    clog: String,
    /// 文件日志级别
    #[clap(long, default_value_t = String::from("TRACE"))]
    flog: String,
    /// `span` 过滤器级别
    #[clap(long, default_value_t = String::from("INFO"))]
    slog: String,
}

impl BuildArgs {
    pub fn build(&self) {
        Self::build_user_apps();
        self.build_kernel();
    }

    pub fn build_user_apps() {
        println!("Building user apps...");
        Cmd::parse("cargo build --package user --release")
            .args(["--target", TARGET_ARCH])
            .args(["--target-dir", "user/target"])
            .env("RUSTFLAGS", "-Clink-arg=-Tuser/src/linker.ld")
            .invoke();
    }

    pub fn build_kernel(&self) {
        println!("Building kernel...");
        Cmd::parse("cargo build --package kernel")
            .args(["--target", TARGET_ARCH])
            .optional_arg(self.release.then_some("--release"))
            .tap_mut(|cmd| {
                if self.profiling {
                    cmd.args(["--features", "profiling"]);
                }
            })
            .env("RUSTFLAGS", "-Clink-arg=-Tcrates/kernel/src/linker.ld")
            .envs([
                ("KERNEL_CLOG", &self.clog),
                ("KERNEL_FLOG", &self.flog),
                ("KERNEL_SLOG", &self.slog),
            ])
            .invoke();
        let kernel_path = format!(
            "target/{TARGET_ARCH}/{}/kernel",
            if self.release { "release" } else { "debug" }
        );
        fs::copy(kernel_path, format!("target/{TARGET_ARCH}/kernel")).unwrap();
    }
}

pub static USER_BINS: LazyLock<Vec<String>> = LazyLock::new(|| {
    fs::read_dir("user/src/bin")
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
