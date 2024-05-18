use hashbrown::HashMap;
use klocks::{RwLock, SpinMutex};
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport};

use super::{
    virtio::{self, HalImpl},
    BLOCK_SIZE,
};

pub struct DiskDriver {
    device: SpinMutex<VirtIOBlk<HalImpl, MmioTransport>>,
    /// 仅用于读的块缓存
    ///
    /// 这里其实可以考虑实现一个 lru 之类的方式乃至类似于 CMU15445 的 `BufferPoolManager` 的东西
    ///
    /// 不过暂时而言，直接使用块缓存的应该只有目录所用的扇区
    caches: RwLock<HashMap<usize, [u8; BLOCK_SIZE]>>,
}

unsafe impl Send for DiskDriver {}
unsafe impl Sync for DiskDriver {}

impl DiskDriver {
    pub fn init() -> Self {
        Self {
            device: SpinMutex::new(virtio::init()),
            caches: RwLock::new(HashMap::new()),
        }
    }

    pub fn read_blocks(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
        if let Err(e) = self.device.lock().read_blocks(block_id, buf) {
            panic!("Failed reading virtio blocks {block_id}: {e}");
        }
    }

    pub fn read_blocks_cached(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
        if let Some(block) = self.caches.read().get(&block_id) {
            buf.copy_from_slice(block);
            return;
        }
        self.read_blocks(block_id, buf);
        self.caches.write().insert(block_id, *buf);
    }

    // pub fn write_blocks(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
    //     if let Err(e) = self.device.lock().write_blocks(block_id, buf) {
    //         panic!("Failed writing virtio blocks {block_id}: {e}");
    //     }
    //     self.caches.write().insert(block_id, *buf);
    // }
}
