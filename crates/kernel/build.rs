fn main() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    println!("cargo::rustc-link-arg={crate_dir}/src/linker.ld");
}
