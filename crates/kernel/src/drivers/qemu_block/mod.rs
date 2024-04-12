use klocks::{Lazy, SpinNoIrqMutex};

use self::disk_driver::DiskDriver;

mod disk_driver;
mod virtio;

pub use disk_driver::SeekFrom;

// TODO: [mid] 关中断锁的可能导致延迟太高
pub static BLOCK_DEVICE: Lazy<SpinNoIrqMutex<DiskDriver>> =
    Lazy::new(|| SpinNoIrqMutex::new(DiskDriver::init()));
