use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
};

use clap::Parser;
use fatfs::{FileSystem, FsOptions};
use tap::Tap;

use crate::{
    build::{BuildArgs, USER_BINS},
    cmd_util::Cmd,
    tool,
    variables::{FS_IMG_ORIGIN_PATH, FS_IMG_PATH, TARGET_ARCH},
    KERNEL_BIN_PATH, KERNEL_ELF_PATH,
};

/// 生成内核或指定 ELF 的汇编
#[derive(Parser)]
pub struct AsmArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// ELF path, if not specified, kernel's path will be selected
    #[clap(short, long)]
    path: Option<PathBuf>,
    #[clap(long)]
    skip_build: bool,
}

impl AsmArgs {
    pub fn dump(self) {
        if !self.skip_build {
            self.build.build_kernel();
        }
        let elf_path = self.path.unwrap_or_else(|| PathBuf::from(KERNEL_ELF_PATH));
        let output = Cmd::parse("rust-objdump --arch-name=riscv64 -g")
            .args([
                "--source",
                "--demangle",
                "--line-numbers",
                "--file-headers",
                // "--section-headers",
                "--symbolize-operands",
                "--print-imm-hex",
                "--no-show-raw-insn",
            ])
            .args(["--section", ".data"])
            .args(["--section", ".bss"])
            .args(["--section", ".text"])
            .args(["--section", ".stack"])
            .args([&elf_path])
            .tap(|cmd| println!("Invoking {:?}", cmd.info()))
            .output();
        fs::write(elf_path.with_extension("S"), output.stdout).unwrap();
        println!(
            "Asm generated at {}",
            elf_path.with_extension("S").display()
        );
    }
}

/// fat-fs 探针。单纯是用于加载和查看 fat 文件系统中的东西
#[derive(Parser)]
pub struct FatProbeArgs {
    #[clap(long)]
    img_path: String,
}

impl FatProbeArgs {
    pub fn probe(&self) {
        let fs = File::options()
            .read(true)
            .write(true)
            .open(&self.img_path)
            .unwrap();
        let fs = FileSystem::new(fs, FsOptions::new()).unwrap();
        let root_dir = fs.root_dir();
        for entry in root_dir.iter() {
            let entry = entry.unwrap();
            println!("{}", entry.file_name());
        }
    }
}

pub fn prepare_env() {
    Cmd::parse(&format!("rustup target add {TARGET_ARCH}")).invoke();
    Cmd::parse("rustup component add llvm-tools-preview").invoke();
    Cmd::parse("cargo install cargo-binutils").invoke();
}

// 将一系列 elf 打包入 fat32 镜像中
pub fn pack(elf_names: &[String]) {
    // 复制一个原始镜像
    let mut origin = File::open(FS_IMG_ORIGIN_PATH).unwrap();
    let mut fs = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(FS_IMG_PATH)
        .unwrap();
    std::io::copy(&mut origin, &mut fs).unwrap();
    fs.seek(SeekFrom::Start(0)).unwrap();
    let fs = FileSystem::new(fs, FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();

    let pack_into = |place_in_host: &str, place: &str| {
        let elf = std::fs::read(place_in_host).unwrap();
        let mut file = root_dir.create_file(place).unwrap();
        file.truncate().unwrap();
        file.write_all(&elf).unwrap();
    };
    for elf_name in elf_names {
        pack_into(
            &format!("user/target/{TARGET_ARCH}/release/{elf_name}"),
            elf_name,
        );
    }
    // pack_into("res/test_bin/clone", "clone");
    // pack_into("res/test_bin/execve", "execve");
    // pack_into("res/test_bin/fork", "fork");
}

pub fn clean() {
    Cmd::parse("cargo clean")
        .invoke()
        .args(["--target-dir", "user/target"])
        .invoke();
}

pub fn lint() {
    Cmd::parse("cargo clippy --workspace --exclude xtask")
        .args(["--target", TARGET_ARCH])
        .invoke();
    Cmd::parse("cargo clippy --package xtask").invoke();
}

/// 准备 OS 运行需要的二进制文件，包括内核二进制和文件镜像
pub fn prepare_os() {
    // Make kernel bin
    println!("Making kernel bin...");
    Cmd::new("rust-objcopy")
        .arg(KERNEL_ELF_PATH)
        .args(["-O", "binary", KERNEL_BIN_PATH])
        .invoke();

    // Pack filesystem
    println!("Packing filesystem...");
    tool::pack(&USER_BINS);
}
