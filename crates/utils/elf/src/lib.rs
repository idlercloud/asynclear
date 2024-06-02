//! 不知道为什么，如果引入了 goblin 的话，当前 crate 的补全速度就会显著降低。
//!
//! 用一个 crate 重导出 goblin 的内容给 kernel 使用，可以缓解问题

#![no_std]

pub use goblin::elf::{
    program_header::{PF_R, PF_W, PF_X, PT_INTERP, PT_LOAD},
    Elf,
};
