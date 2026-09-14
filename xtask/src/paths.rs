use std::{path::PathBuf, sync::OnceLock};

use serde_json::Value;

use crate::{cmd_util::Cmd, variables::TARGET_ARCH};

/// 查询并缓存 `cargo metadata`。
///
/// 产物目录可能被 `.cargo/config.toml` 的 `build.target-dir` 或 `CARGO_TARGET_DIR`
/// 环境变量改写，因此不能假定它一定是 `<workspace>/target`。
fn metadata() -> &'static Value {
    static METADATA: OnceLock<Value> = OnceLock::new();
    METADATA.get_or_init(|| {
        let output = Cmd::parse("cargo metadata --format-version 1 --no-deps").output();
        serde_json::from_slice(&output.stdout).expect("Failed to parse cargo metadata output")
    })
}

fn metadata_path(field: &str) -> PathBuf {
    metadata()[field]
        .as_str()
        .unwrap_or_else(|| panic!("Missing `{field}` in cargo metadata output"))
        .into()
}

/// cargo 实际使用的 target 目录。
pub fn target_dir() -> PathBuf {
    metadata_path("target_directory")
}

/// 用户程序 ELF 所在的目录。
pub fn user_bin_dir() -> PathBuf {
    target_dir().join(TARGET_ARCH).join("release")
}

/// 内核 ELF，即 `build` 阶段从 profile 子目录拷贝出来的那一份。
pub fn kernel_elf() -> PathBuf {
    target_dir().join(TARGET_ARCH).join("kernel")
}

/// 内核二进制，由 `rust-objcopy` 从 [`kernel_elf`] 生成。
pub fn kernel_bin() -> PathBuf {
    kernel_elf().with_extension("bin")
}
