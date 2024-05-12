use clap::Parser;
use const_format::formatcp;

use crate::{
    build::BuildArgs,
    cmd_util::Cmd,
    tool,
    variables::{FS_IMG_PATH, SBI_PATH},
    KERNEL_BIN_PATH,
};

/// 使用 QEMU 运行内核
#[derive(Parser)]
pub struct QemuArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Hart 数量（SMP 代表 Symmetrical Multiple Processor）.
    #[clap(long, default_value_t = 2)]
    smp: u8,
    /// 如果开启，QEMU 会阻塞并等待 GDB 连接
    #[clap(long)]
    debug: bool,
}

impl QemuArgs {
    pub fn run(self) {
        // 构建内核和用户应用
        self.build.build();
        tool::prepare_os();

        println!("Running qemu...");

        Self::base_qemu()
            .args(["-smp", &self.smp.to_string()])
            .optional_arg(self.debug.then_some("-s"))
            .optional_arg(self.debug.then_some("-S"))
            .invoke();
    }

    pub fn base_qemu() -> Cmd {
        let mut cmd = Cmd::new("qemu-system-riscv64");
        cmd.args(["-machine", "virt"])
            .args(["-kernel", KERNEL_BIN_PATH])
            .args(["-m", "128M"])
            .args(["-nographic"])
            .args(["-bios", SBI_PATH])
            .args([
                "-drive",
                formatcp!("file={FS_IMG_PATH},if=none,format=raw,id=x0"),
            ])
            .args([
                "-device",
                "virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0",
            ]);
        cmd
    }
}
