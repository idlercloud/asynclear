use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport};

use super::{
    virtio::{self, HalImpl},
    BlockDevice,
};

type VirtIOBlockDevicde = VirtIOBlk<HalImpl, MmioTransport>;

pub struct DiskDriver {
    device: VirtIOBlockDevicde,
    block_id: u64,
    block_offset: u32,
    block_buffer: [u8; VirtIOBlockDevicde::BLOCK_SIZE as usize],
}

unsafe impl Send for DiskDriver {}
unsafe impl Sync for DiskDriver {}

impl core::fmt::Write for DiskDriver {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

impl DiskDriver {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            device: virtio::new(),
            block_id: 0,
            block_offset: 0,
            block_buffer: [0; VirtIOBlockDevicde::BLOCK_SIZE as usize],
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut tot_nread = 0;
        while tot_nread < buf.len() {
            let copy_len = usize::min(
                buf.len() - tot_nread,
                (VirtIOBlockDevicde::BLOCK_SIZE - self.block_offset) as usize,
            );
            self.device
                .read_block(self.block_id, &mut self.block_buffer);
            buf[tot_nread..tot_nread + copy_len].copy_from_slice(
                &self.block_buffer
                    [self.block_offset as usize..self.block_offset as usize + copy_len],
            );
            self.block_offset += copy_len as u32;
            if self.block_offset % VirtIOBlockDevicde::BLOCK_SIZE == 0 {
                self.block_offset = 0;
                self.block_id += 1;
            }
            tot_nread += copy_len;
        }
        tot_nread
    }

    pub fn write(&mut self, buf: &[u8]) -> usize {
        let mut tot_write = 0;
        while tot_write < buf.len() {
            let copy_len = usize::min(
                buf.len() - tot_write,
                (VirtIOBlockDevicde::BLOCK_SIZE - self.block_offset) as usize,
            );
            if copy_len != VirtIOBlockDevicde::BLOCK_SIZE as usize {
                self.device
                    .read_block(self.block_id, &mut self.block_buffer);
                self.block_buffer
                    [self.block_offset as usize..self.block_offset as usize + copy_len]
                    .copy_from_slice(&buf[tot_write..tot_write + copy_len]);
                self.device.write_block(self.block_id, &self.block_buffer);
            } else {
                self.device.write_block(
                    self.block_id,
                    &buf[tot_write..tot_write + copy_len].try_into().unwrap(),
                );
            }
            self.block_offset += copy_len as u32;
            if self.block_offset % VirtIOBlockDevicde::BLOCK_SIZE == 0 {
                self.block_offset = 0;
                self.block_id += 1;
            }
            tot_write += copy_len;
        }
        tot_write
    }

    pub fn seek(&mut self, pos: SeekFrom) -> u64 {
        let offset = match pos {
            SeekFrom::Start(from_start) => from_start,
            SeekFrom::End(from_end) => {
                let end_offset = self.device.capacity() * VirtIOBlockDevicde::BLOCK_SIZE as u64 - 1;
                let offset = end_offset.checked_add_signed(from_end).unwrap();
                offset
            }
            SeekFrom::Current(from_current) => {
                let curr_offset = self.block_id * VirtIOBlockDevicde::BLOCK_SIZE as u64
                    + self.block_offset as u64;
                let offset = curr_offset.checked_add_signed(from_current).unwrap();
                offset
            }
        };
        self.block_id = offset / VirtIOBlockDevicde::BLOCK_SIZE as u64;
        self.block_offset = (offset % VirtIOBlockDevicde::BLOCK_SIZE as u64) as u32;
        offset
    }
}

/// Enumeration of possible methods to seek within an I/O object.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes.
    Start(u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    End(i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    Current(i64),
}
