//! fat32 文件系统的实现。
//!
//! 可以参考：
//! - <https://wiki.osdev.org/FAT/>
//! - <https://www.win.tue.nl/~aeb/linux/fs/fat/fat-1.html/>
//! - <https://github.com/rafalh/rust-fatfs/>
//! - <https://elm-chan.org/docs/fat_e.html>

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(iter_array_chunks)]
#![feature(coroutines, iter_from_coroutine)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(decl_macro)]

extern crate alloc;

#[macro_use]
extern crate kernel_tracer;

#[cfg(all(feature = "std", feature = "kernel"))]
compile_error!("Feature `std` 与 `kernel` 互斥，只能开启其中之一");

mod bpb;
mod dir_entry;
mod fat;

pub use bpb::BiosParameterBlock;
pub use dir_entry::{DirEntry, DirEntryBuilder, DirEntryBuilderResult, DIR_ENTRY_SIZE};
pub use fat::FileAllocTable;

pub const SECTOR_SIZE: usize = 512;
pub const BOOT_SECTOR_ID: usize = 0;
