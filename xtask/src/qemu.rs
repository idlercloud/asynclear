use std::{
    env, fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use const_format::formatcp;

use crate::{
    build::BuildArgs,
    cmd_util::Cmd,
    timing::TimerSession,
    tool,
    variables::{FS_IMG_PATH, SBI_PATH},
    KERNEL_BIN_PATH,
};

/// 使用 QEMU 运行内核
#[derive(Parser)]
pub struct QemuArgs {
    #[clap(flatten)]
    build_args: BuildArgs,
    /// Hart 数量（SMP 代表 Symmetrical Multiple Processor）.
    #[clap(long, default_value_t = 2)]
    smp: u8,
    #[clap(long)]
    skip_build: bool,
    /// 如果开启，QEMU 会阻塞并等待 GDB 连接
    #[clap(long)]
    debug: bool,
    /// 将内核文件日志输出到宿主机文件
    #[clap(long)]
    virtio_log_file: Option<PathBuf>,
}

impl QemuArgs {
    pub fn run(self) {
        // 构建内核和用户应用
        if !self.skip_build {
            let _session = TimerSession::start();
            self.build_args.build();
            tool::prepare_os();
        }

        println!("Running qemu...");

        let mut cmd = Self::base_qemu();
        let virtio_log_file = self
            .virtio_log_file
            .or_else(|| kernel_file_log_enabled().then(|| PathBuf::from(DEFAULT_VIRTIO_LOG_PATH)));
        if let Some(path) = virtio_log_file.as_deref() {
            add_virtio_log_file(&mut cmd, path);
            println!("log file location: {}", path.display());
        }

        cmd.args(["-smp", &self.smp.to_string()])
            .optional_args(self.debug.then_some(["-s", "-S"]))
            .invoke();
    }

    pub fn base_qemu() -> Cmd {
        let mut cmd = Cmd::new("qemu-system-riscv64");
        cmd.args(["-machine", "virt", "-m", "128M", "-nographic"])
            .args(["-kernel", KERNEL_BIN_PATH, "-bios", SBI_PATH])
            .args(["-drive", formatcp!("file={FS_IMG_PATH},if=none,format=raw,id=x0")])
            .args(["-device", "virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0"]);
        cmd
    }
}

const DEFAULT_VIRTIO_LOG_PATH: &str = "logs/kernel.log";

fn kernel_file_log_enabled() -> bool {
    matches!(
        env::var("KERNEL_FLOG").ok().as_deref(),
        Some("TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR")
    )
}

fn add_virtio_log_file(cmd: &mut Cmd, path: &Path) {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).unwrap();
    }

    cmd.args(["-chardev", &format!("file,id=kernel_log,path={}", path.display())])
        .args(["-device", "virtio-serial-device,id=logserial,bus=virtio-mmio-bus.1"])
        .args(["-device", "virtconsole,bus=logserial.0,chardev=kernel_log"]);
}
