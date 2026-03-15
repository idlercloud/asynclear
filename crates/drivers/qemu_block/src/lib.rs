#![no_std]

mod disk_driver;
mod virtio;

use klocks::Lazy;

pub use self::disk_driver::DiskDriver;

pub static BLOCK_DEVICE: Lazy<DiskDriver> = Lazy::new(DiskDriver::init);
