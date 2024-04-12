use fatfs::{Dir, LossyOemCpConverter, NullTimeProvider};

use super::BlockDeviceWrapper;

pub struct FatDir {
    inner: Dir<'static, BlockDeviceWrapper, NullTimeProvider, LossyOemCpConverter>,
}
