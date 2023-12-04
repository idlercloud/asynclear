use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

use chrono::{FixedOffset, Utc};
use clap::Parser;

use crate::{
    build::{BuildArgs, USER_BINS},
    cmd_util::Cmd,
    tool,
    variables::SBI_PATH,
    KERNEL_BIN_PATH, KERNEL_ELF_PATH,
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
        // Build kernel and user apps
        self.build.build();

        // Make kernel bin
        println!("Making kernel bin...");
        make_bin(KERNEL_ELF_PATH);

        // Pack filesystem
        println!("Packing filesystem...");
        tool::pack(&USER_BINS);

        // Call qemu
        println!("Running qemu...");
        fs::create_dir_all("logs").unwrap();
        let date_time = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
        let log_file_name = format!("logs/{}.log", date_time.format("%Y-%m-%d %H_%M_%S"));
        // 预留 512KB 的日志空间
        const LOG_PRESERVED_SIZE: u64 = 512 * 1024;
        {
            let mut log_file = File::create(&log_file_name).unwrap();
            log_file.set_len(LOG_PRESERVED_SIZE).unwrap();
            let placeholder = vec![b' '; LOG_PRESERVED_SIZE as usize];
            log_file.write_all(&placeholder).unwrap();
        }
        Cmd::new("qemu-system-riscv64")
            .args(["-machine", "virt"])
            .args(["-kernel", KERNEL_BIN_PATH])
            .args(["-m", "128M"])
            .args(["-nographic"])
            .args(["-smp", &self.smp.to_string()])
            .args(["-bios", SBI_PATH])
            // .args(&[
            //     "-drive",
            //     &format!("file={FS_IMG_PATH},if=none,format=raw,id=x0"),
            // ])
            .args([
                "-drive",
                &format!("file={log_file_name},if=none,format=raw,id=x0"),
            ])
            .args([
                "-device",
                "virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0",
            ])
            .optional_arg(self.debug.then_some("-s"))
            .optional_arg(self.debug.then_some("-S"))
            .invoke();

        let mut log_file = File::options()
            .read(true)
            .write(true)
            .open(&log_file_name)
            .unwrap();
        let mut log_bytes = Vec::with_capacity(LOG_PRESERVED_SIZE as usize);
        #[allow(clippy::verbose_file_reads)]
        log_file.read_to_end(&mut log_bytes).unwrap();
        let mut len = LOG_PRESERVED_SIZE;
        for byte in log_bytes.into_iter().rev() {
            if byte != b' ' {
                break;
            }
            len -= 1;
        }
        log_file.set_len(len).unwrap();
    }
}

fn make_bin(elf_path: impl AsRef<Path>) {
    let path = elf_path.as_ref();
    Cmd::new("rust-objcopy")
        .arg(path)
        .args(["-O", "binary"])
        .arg(path.with_extension("bin"))
        .invoke();
}
