use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    iter,
    path::PathBuf,
};

use clap::Parser;
use fastrand::Rng;
use fatfs::{Dir, FileSystem, FsOptions};
use tap::Tap;

use crate::{
    build::{BuildArgs, PTEST_BINS, USER_BINS},
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
    /// ELF 路径，如果未指定则使用 kernel 的路径
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
        let output = Cmd::parse("rust-objdump --arch-name=riscv64 --mattr=+d -g")
            .args([
                "--source",
                "--demangle",
                "--line-numbers",
                "--file-headers",
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
    #[clap(long)]
    file_path: Option<String>,
}

impl FatProbeArgs {
    pub fn probe(&self) {
        let fs = File::options()
            .read(true)
            .write(true)
            .open(&self.img_path)
            .unwrap();
        let fs = FileSystem::new(fs, FsOptions::new()).unwrap();
        if let Some(file_path) = &self.file_path {
            let mut dir = fs.root_dir();
            let components = file_path.split('/').collect::<Vec<_>>();
            for &component in &components[0..components.len() - 1] {
                dir = dir.open_dir(component).unwrap();
            }
            let last_component = components.last().unwrap();
            if let Ok(dir) = dir.open_dir(last_component) {
                for entry in dir.iter() {
                    let entry = entry.unwrap();
                    let name = entry.file_name();
                    if name != "." && name != ".." {
                        println!("{name}");
                    }
                }
            } else if let Ok(mut file) = dir.open_file(last_component) {
                let mut target = File::create(format!("res/{last_component}")).unwrap();
                io::copy(&mut file, &mut target).unwrap();
            }
        } else {
            fn walk_dir(curr: Dir<'_, File>, depth: usize) {
                for entry in curr.iter() {
                    let entry = entry.unwrap();
                    let name = entry.file_name();
                    if name != "." && name != ".." {
                        for _ in 0..depth {
                            print!("  ");
                        }
                        println!("{name}");
                    }
                    if entry.is_dir() && name != "." && name != ".." {
                        let child = entry.to_dir();
                        walk_dir(child, depth + 1);
                    }
                }
            }
            let root_dir = fs.root_dir();
            walk_dir(root_dir, 0);
        }
    }
}

pub fn prepare_env() {
    Cmd::parse(&format!("rustup target add {TARGET_ARCH}")).invoke();
    Cmd::parse("rustup component add llvm-tools-preview").invoke();
    Cmd::parse("cargo install cargo-binutils").invoke();
}

// 将一系列 elf 打包入 fat32 镜像中
pub fn pack() {
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

    let pack_into = |place_in_host: &str, path: &str| {
        let elf = fs::read(place_in_host).expect(place_in_host);
        let mut dir = root_dir.clone();
        let components = path.split('/').collect::<Vec<_>>();
        for &component in &components[0..components.len() - 1] {
            dir = dir.create_dir(component).unwrap();
        }
        let mut file = dir.create_file(components[components.len() - 1]).unwrap();
        file.truncate().unwrap();
        file.write_all(&elf).unwrap();
    };
    for elf_name in USER_BINS.iter() {
        let src_path = format!("target/{TARGET_ARCH}/release/{elf_name}");
        if elf_name.starts_with("test_") {
            pack_into(&src_path, &format!("ktest/{elf_name}"));
        } else if elf_name.starts_with("bench_") || elf_name == "_empty" {
            pack_into(&src_path, &format!("kbench/{elf_name}"));
        } else {
            pack_into(&src_path, elf_name);
        }
    }
    for ptest_name in PTEST_BINS.iter() {
        pack_into(
            &format!("res/preliminary/{ptest_name}"),
            &format!("ptest/{ptest_name}"),
        );
    }
    if PTEST_BINS.len() > 0 {
        pack_into("res/preliminary/text.txt", "ptest/text.txt");
        pack_into("res/preliminary/mnt/test_mount", "ptest/mnt/test_mount");
    }
    {
        let mut pg = root_dir
            .open_dir("kbench")
            .unwrap()
            .create_file("_playground")
            .unwrap();
        let mut rng = Rng::with_seed(19260817);
        let buf = iter::repeat_with(|| rng.u8(0..=255))
            .take(1024 * 1234)
            .collect::<Vec<u8>>();
        pg.write_all(&buf).unwrap();
    }
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
    tool::pack();
}
