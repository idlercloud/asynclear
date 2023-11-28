use clap::Parser;
use const_format::formatcp;
use fatfs::{FileSystem, FsOptions};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tap::Tap;

use crate::cmd_util::Cmd;
use crate::variables::{FS_IMG_ORIGIN_PATH, FS_IMG_PATH, TARGET_ARCH};
use crate::BINARY_DIR;

#[derive(Parser)]
pub struct AsmArgs {
    /// ELF path, if not specified, kernel's path will be selected
    #[clap(short, long)]
    path: Option<PathBuf>,
}

impl AsmArgs {
    pub fn dump(self) {
        let elf_path = self
            .path
            .unwrap_or_else(|| PathBuf::from(formatcp!("{BINARY_DIR}/kernel")));
        let output = Cmd::cmd("rust-objdump --arch-name=riscv64 -S")
            .args(&["--section", ".data"])
            .args(&["--section", ".bss"])
            .args(&["--section", ".text"])
            .args(&[&elf_path])
            .tap(|cmd| println!("Invoking {:?}", cmd.info()))
            .output();
        fs::write(elf_path.with_extension("S"), output.stdout).unwrap();
        println!(
            "Asm generated at {}",
            elf_path.with_extension("S").display()
        );
    }
}

pub fn elf_extract() {
    let fs = File::options()
        .read(true)
        .write(true)
        .open("res/fat32.img")
        .unwrap();

    let fs = FileSystem::new(fs, FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut data = Vec::new();
    root_dir
        .open_file("lua")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    fs::write("lua.elf", data).unwrap();
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
    Cmd::cmd(formatcp!("rustup target add {TARGET_ARCH}")).invoke();
    Cmd::cmd("rustup component add llvm-tools-preview").invoke();
    Cmd::cmd("cargo install cargo-binutils").invoke();
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
        pack_into(&format!("user/{BINARY_DIR}/{elf_name}"), &elf_name);
    }
    pack_into("res/test_bin/clone", "clone");
    pack_into("res/test_bin/execve", "execve");
    pack_into("res/test_bin/fork", "fork");
}

pub fn clean() {
    Cmd::cmd("cargo clean")
        .invoke()
        .args(&["--target-dir", "user/target"])
        .invoke();
}

pub fn lint() {
    Cmd::cmd("cargo clippy --package kernel")
        .args(&["--target", TARGET_ARCH])
        .invoke();
}
