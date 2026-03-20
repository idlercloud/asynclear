#![no_std]

mod disk_driver;
mod hal_impl;

pub use disk_driver::DiskDriver;
pub use hal_impl::HalImpl;
