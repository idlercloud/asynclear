use hal::block_device::{BlockDevice, BLOCK_SIZE};
use hashbrown::HashMap;
use klocks::{RwLock, SpinMutex};
use virtio_drivers::{device::blk::VirtIOBlk, transport::Transport, Hal};

pub struct DiskDriver<H: Hal, T: Transport> {
    device: SpinMutex<VirtIOBlk<H, T>>,
    /// 仅用于读的块缓存
    ///
    /// 这里其实可以考虑实现一个 lru 之类的方式乃至类似于 CMU15445 的 `BufferPoolManager` 的东西
    ///
    /// 不过暂时而言，直接使用块缓存的应该只有目录所用的扇区
    caches: RwLock<HashMap<usize, [u8; BLOCK_SIZE]>>,
}

impl<H: Hal, T: Transport> DiskDriver<H, T> {
    pub fn new(block_device: VirtIOBlk<H, T>) -> Self {
        Self {
            device: SpinMutex::new(block_device),
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

// TODO: 实现可失败的 read_blocks/write_blocks

impl<H: Hal + Send + Sync, T: Transport + Send + Sync> BlockDevice for DiskDriver<H, T> {
    fn read_block(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
        self.read_blocks(block_id, buf);
    }

    fn read_block_cached(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) {
        self.read_blocks_cached(block_id, buf);
    }
}
