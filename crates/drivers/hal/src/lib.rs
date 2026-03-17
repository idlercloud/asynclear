#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "std", feature = "kernel"))]
compile_error!("Feature `std` 与 `kernel` 互斥，只能开启其中之一");

pub mod block_device {
    use klocks::Once;

    pub const BLOCK_SIZE: usize = 512;

    pub trait BlockDevice: Send + Sync {
        fn read_block(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]);

        fn read_block_cached(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
            self.read_block(block_id, buf);
        }
    }

    static BLOCK_DEVICE: Once<&'static dyn BlockDevice> = Once::new();

    pub fn init_instance(device: &'static dyn BlockDevice) {
        BLOCK_DEVICE.call_once(|| device);
    }

    pub fn instance() -> &'static dyn BlockDevice {
        *BLOCK_DEVICE.get().unwrap()
    }
}
