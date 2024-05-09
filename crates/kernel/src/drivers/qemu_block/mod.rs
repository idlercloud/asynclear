use klocks::Lazy;

mod disk_driver;
mod virtio;

pub use self::disk_driver::DiskDriver;

pub static BLOCK_DEVICE: Lazy<DiskDriver> = Lazy::new(DiskDriver::init);
pub const BLOCK_SIZE: usize = 512;
