#![no_std]
#![feature(iter_array_chunks)]
#![feature(coroutines, iter_from_coroutine)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(decl_macro)]

extern crate alloc;

#[macro_use]
extern crate kernel_tracer;

mod bpb;
mod dir_entry;
mod fat;

pub use bpb::BiosParameterBlock;
pub use dir_entry::{DirEntry, DirEntryBuilder, DirEntryBuilderResult, DIR_ENTRY_SIZE};
pub use fat::FileAllocTable;

pub const SECTOR_SIZE: usize = 512;
pub const BOOT_SECTOR_ID: usize = 0;

pub trait BlockDevice: Send + Sync {
    fn read_block(&self, block_id: usize, buf: &mut [u8; SECTOR_SIZE]);

    fn read_block_cached(&self, block_id: usize, buf: &mut [u8; SECTOR_SIZE]) {
        self.read_block(block_id, buf);
    }
}
