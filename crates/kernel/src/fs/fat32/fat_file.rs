use defines::error::{errno, Result};
use fatfs::{LossyOemCpConverter, NullTimeProvider, Read, Write};
use klocks::SpinMutex;
use user_check::{UserCheck, UserCheckMut};

use super::BlockDeviceWrapper;

type RawFatFile = fatfs::File<'static, BlockDeviceWrapper, NullTimeProvider, LossyOemCpConverter>;

pub struct FatFile {
    inner: SpinMutex<RawFatFile>,
    readable: bool,
    writable: bool,
}

impl FatFile {
    pub fn read(&self, buf: UserCheckMut<[u8]>) -> Result<usize> {
        let mut buf = buf.check_slice_mut()?;
        self.inner.lock().read(&mut buf).map_err(|_| errno::EIO)
    }

    pub fn write(&self, buf: UserCheck<[u8]>) -> Result<usize> {
        let buf = buf.check_slice()?;
        self.inner.lock().write(&buf).map_err(|_| errno::EIO)
    }
}
