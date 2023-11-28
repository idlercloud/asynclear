pub mod disk_driver;
pub mod virtio;

/// 块设备的抽象，读写都以块为单位进行
pub trait BlockDevice {
    const BLOCK_SIZE: u32;
    fn read_block(&mut self, block_id: u64, buf: &mut [u8; Self::BLOCK_SIZE as usize]);
    fn write_block(&mut self, block_id: u64, buf: &[u8; Self::BLOCK_SIZE as usize]);
}
