use std::{fs, sync::LazyLock};

use clap::Parser;

use crate::{cmd_util::Cmd, paths, timing::ScopedTimer, variables::TARGET_ARCH};

/// 构建内核和用户程序
#[derive(Parser)]
pub struct BuildArgs {
    /// 是否以 release 构建内核
    #[clap(long)]
    release: bool,
    /// 是否开启 profiling
    #[clap(long)]
    profiling: bool,
}

impl BuildArgs {
    pub fn build(&self) {
        let _timer = ScopedTimer::start("build all");
        self.build_user_apps();
        self.build_kernel();
    }

    fn build_user_apps(&self) {
        println!("Building user apps...");
        let _timer = ScopedTimer::start("build user apps");
        Cmd::parse("cargo build --package user --release")
            .args(["--target", TARGET_ARCH])
            .invoke();
    }

    pub fn build_kernel(&self) {
        println!("Building kernel...");
        let _timer = ScopedTimer::start("build kernel");
        Cmd::parse("cargo build --package kernel")
            .args(["--target", TARGET_ARCH])
            .optional_arg(self.release.then_some("--release"))
            .optional_args(self.profiling.then_some(["--features", "profiling"]))
            .invoke();
        let profile = if self.release { "release" } else { "debug" };
        let built_kernel = paths::target_dir().join(TARGET_ARCH).join(profile).join("kernel");
        fs::copy(built_kernel, paths::kernel_elf()).unwrap();
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
