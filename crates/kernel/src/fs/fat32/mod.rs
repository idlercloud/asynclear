// mod fat_dir;
// mod fat_file;

// use defines::error::{errno, Error};
// use fatfs::{FileSystem, FsOptions, LossyOemCpConverter, NullTimeProvider};
// use klocks::Lazy;

// use crate::drivers::qemu_block::{SeekFrom, BLOCK_DEVICE};

// pub struct FatFs {
//     fs: FileSystem<BlockDeviceWrapper, NullTimeProvider, LossyOemCpConverter>,
// }

// // TODO: [mid] 暂时这么做，可能有问题
// unsafe impl Sync for FatFs {}

// pub static FAT_FS: Lazy<FatFs> = Lazy::new(|| FatFs {
//     fs: FileSystem::new(BlockDeviceWrapper(()), FsOptions::new()).unwrap(),
// });

// struct BlockDeviceWrapper(());

// #[derive(Debug)]
// struct ErrorWrapper(Error);

// impl From<ErrorWrapper> for Error {
//     fn from(value: ErrorWrapper) -> Self {
//         value.0
//     }
// }

// impl fatfs::IoError for ErrorWrapper {
//     fn is_interrupted(&self) -> bool {
//         false
//     }
//     fn new_unexpected_eof_error() -> Self {
//         Self(errno::EIO)
//     }
//     fn new_write_zero_error() -> Self {
//         Self(errno::EIO)
//     }
// }

// impl fatfs::IoBase for BlockDeviceWrapper {
//     type Error = ErrorWrapper;
// }

// impl fatfs::Read for BlockDeviceWrapper {
//     fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
//         BLOCK_DEVICE.lock().read(buf).map_err(ErrorWrapper)
//     }
// }

// impl fatfs::Write for BlockDeviceWrapper {
//     fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
//         BLOCK_DEVICE.lock().write(buf).map_err(ErrorWrapper)
//     }
//     fn flush(&mut self) -> Result<(), Self::Error> {
//         BLOCK_DEVICE.lock().flush().map_err(ErrorWrapper)
//     }
// }

// impl fatfs::Seek for BlockDeviceWrapper {
//     fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
//         let pos = match pos {
//             fatfs::SeekFrom::Start(i) => SeekFrom::Start(i),
//             fatfs::SeekFrom::End(i) => SeekFrom::End(i),
//             fatfs::SeekFrom::Current(i) => SeekFrom::Current(i),
//         };
//         Ok(BLOCK_DEVICE.lock().seek(pos))
//     }
// }
