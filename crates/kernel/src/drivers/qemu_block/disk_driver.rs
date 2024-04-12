use super::virtio::{self, HalImpl};

use defines::error::{errno, Result};
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport};

const BLOCK_SIZE: usize = 512;

pub struct DiskDriver {
    device: VirtIOBlk<HalImpl, MmioTransport>,
    block_id: usize,
    block_offset: u32,
    block_buffer: [u8; BLOCK_SIZE],
}

unsafe impl Send for DiskDriver {}
unsafe impl Sync for DiskDriver {}

impl DiskDriver {
    pub fn init() -> Self {
        Self {
            device: virtio::init(),
            block_id: 0,
            block_offset: 0,
            block_buffer: [0; BLOCK_SIZE as usize],
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut tot_nread = 0;
        while tot_nread < buf.len() {
            let copy_len = usize::min(
                buf.len() - tot_nread,
                BLOCK_SIZE - self.block_offset as usize,
            );
            self.device
                .read_blocks(self.block_id, &mut self.block_buffer)
                .map_err(|_| errno::EIO)?;
            buf[tot_nread..tot_nread + copy_len].copy_from_slice(
                &self.block_buffer
                    [self.block_offset as usize..self.block_offset as usize + copy_len],
            );
            self.block_offset += copy_len as u32;
            if self.block_offset % BLOCK_SIZE as u32 == 0 {
                self.block_offset = 0;
                self.block_id += 1;
            }
            tot_nread += copy_len;
        }
        Ok(tot_nread)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut tot_write = 0;
        while tot_write < buf.len() {
            let copy_len = usize::min(
                buf.len() - tot_write,
                (BLOCK_SIZE as u32 - self.block_offset) as usize,
            );
            if copy_len != BLOCK_SIZE {
                self.device
                    .read_blocks(self.block_id, &mut self.block_buffer)
                    .map_err(|_| errno::EIO)?;
                self.block_buffer
                    [self.block_offset as usize..self.block_offset as usize + copy_len]
                    .copy_from_slice(&buf[tot_write..tot_write + copy_len]);
                self.device
                    .write_blocks(self.block_id, &self.block_buffer)
                    .map_err(|_| errno::EIO)?;
            } else {
                self.device
                    .write_blocks(self.block_id, &buf[tot_write..tot_write + copy_len])
                    .map_err(|_| errno::EIO)?;
            }
            self.block_offset += copy_len as u32;
            if self.block_offset % BLOCK_SIZE as u32 == 0 {
                self.block_offset = 0;
                self.block_id += 1;
            }
            tot_write += copy_len;
        }
        Ok(tot_write)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> u64 {
        let offset = match pos {
            SeekFrom::Start(from_start) => from_start,
            SeekFrom::End(from_end) => {
                let end_offset = self.device.capacity() * BLOCK_SIZE as u64 - 1;
                end_offset.checked_add_signed(from_end).unwrap()
            }
            SeekFrom::Current(from_current) => {
                let curr_offset = (self.block_id * BLOCK_SIZE) as u64 + self.block_offset as u64;
                curr_offset.checked_add_signed(from_current).unwrap()
            }
        };
        self.block_id = offset as usize / BLOCK_SIZE;
        self.block_offset = (offset % BLOCK_SIZE as u64) as u32;
        offset
    }

    pub fn flush(&mut self) -> Result<()> {
        self.device.flush().map_err(|_| errno::EIO)
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
