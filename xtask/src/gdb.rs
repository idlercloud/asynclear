use std::{ffi::OsString, process::Command};

use clap::Parser;

use crate::{cmd_util::Cmd, paths};

/// 用 GDB 连接正在等待调试器的内核
#[derive(Parser)]
pub struct GdbArgs {
    /// gdb 可执行文件路径
    #[clap(long, default_value = "riscv64-unknown-elf-gdb")]
    gdb: OsString,
    /// GDB 服务端口，与 `cargo qemu --debug` 的 `-s` 一致
    #[clap(long, default_value_t = 1234)]
    port: u16,
}

impl GdbArgs {
    pub fn run(self) {
        let elf = paths::kernel_elf();
        if !elf.exists() {
            panic!("Kernel ELF not found at {}, run `cargo xbuild` first", elf.display());
        }
        if let Err(e) = Command::new(&self.gdb).arg("--version").output() {
            panic!(
                "Cannot run `{}`: {e}, install it or specify its path with `--gdb`",
                self.gdb.to_string_lossy()
            );
        }
        println!("Debugging {} with {}", elf.display(), self.gdb.to_string_lossy());
        Cmd::new(&self.gdb)
            .arg(&elf)
            .args(["-ex", "set arch riscv:rv64"])
            .arg("-ex")
            .arg(format!("target remote localhost:{}", self.port))
            .invoke();
    }
}
