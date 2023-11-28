#![no_std]
#![allow(incomplete_features)]
#![feature(strict_provenance)]
#![feature(generic_const_exprs)]

extern crate alloc;

mod block;

pub use self::block::disk_driver::DiskDriver;

pub fn new_virtio_driver() -> DiskDriver {
    DiskDriver::new()
}
