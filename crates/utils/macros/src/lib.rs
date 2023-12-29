use std::fs;

use proc_macro::TokenStream;

#[proc_macro]
pub fn gen_tmp_fs(_items: TokenStream) -> TokenStream {
    let mut array_code = "&[\n".to_string();

    const PREFIX: &str = "../../../../user/target/riscv64imac-unknown-none-elf/release";
    let mut add_elf = |name: &str| {
        use std::fmt::Write;
        writeln!(
            array_code,
            r#"("{name}", ::core::include_bytes!("{PREFIX}/{name}")),"#
        )
        .unwrap();
    };

    add_elf("initproc");
    add_elf("shell");

    for entry in fs::read_dir("user/src/bin").unwrap() {
        let entry = entry.unwrap();
        let elf_name = entry.file_name();
        let elf_name = elf_name.to_str().unwrap().trim_end_matches(".rs");
        if elf_name == "initproc" || elf_name == "shell" {
            continue;
        }

        add_elf(elf_name);
    }

    array_code.push(']');
    array_code.parse().unwrap()
}
