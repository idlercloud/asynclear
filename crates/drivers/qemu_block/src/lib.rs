#![no_std]

mod disk_driver;
mod virtio;

use klocks::Lazy;

pub use self::disk_driver::DiskDriver;

pub static BLOCK_DEVICE: Lazy<DiskDriver<'static>> = Lazy::new(DiskDriver::init);
