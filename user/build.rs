use std::{env, fmt::Write, fs, path::Path};

fn main() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    println!("cargo::rustc-link-arg={crate_dir}/src/linker.ld");

    // 指定要列举的目录
    let dir = "src/bin"; // 你可以替换为任何目录

    // 获取目录下的所有文件名
    let mut codegen_content = String::from("pub const KTESTS: &[&::core::ffi::CStr] = &[\n");
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            continue;
        }
        let path = entry.path();
        if path.starts_with("test_") {
            let test_name = path.file_stem().unwrap().to_str().unwrap();
            writeln!(codegen_content, "    c\"{test_name}\",").unwrap();
        }
    }
    codegen_content.push_str("];\n");

    // 将文件名写入一个 Rust 文件
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("ktest_list.rs");
    fs::write(&dest_path, codegen_content).unwrap();
}
