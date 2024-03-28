use defines::error::Result;
use user_check::{UserCheck, UserCheckMut};

use crate::{fs::TtyFuture, thread::BlockingFuture, uart_console::print};

// /// 操纵某个特殊文件的底层设备。目前只进行错误检验
// ///
// /// 参数：
// /// - `fd` 文件描述符
// /// - `request` 请求码，其含义依赖于设备。包含参数是入参还是出参，以及 argp 指向的大小
// /// - `argp` 一个指针
// ///
// /// 参考：<https://man7.org/linux/man-pages/man2/ioctl.2.html>
// pub fn sys_ioctl(fd: usize, request: usize, argp: usize) -> Result {
//     // // FIXME: 完善 sys_ioctl 的语义
//     // info!("sys_ioctl: fd: {fd}, request: {request}, argp: {argp:#x}");
//     // if !matches!(curr_process().inner().fd_table.get(fd), Some(Some(_))) {
//     //     return Err(errno::EBADF);
//     // }
//     // if curr_page_table().trans_va_to_pa(VirtAddr(argp)).is_none() {
//     //     return Err(errno::EFAULT);
//     // }
//     // Ok(0)
//     todo!("[blocked] sys_ioctl")
// }

// pub fn sys_mkdirat(dirfd: usize, path: *const u8, mode: usize) -> Result {
//     // let path = unsafe { check_cstr(path)? };

//     // info!("mkdir {dirfd}, {path}, {mode:#o}");

//     // let absolute_path = path_with_fd(dirfd, path)?;
//     // // FIXME: 目前这个语义是错误的，创建目录要抽象出另一个函数来
//     // let inode = open_file(absolute_path, OpenFlags::O_CREAT | OpenFlags::O_DIRECTORY)?;
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // let fd = inner.alloc_fd();
//     // inner.fd_table[fd] = Some(Arc::new(inode));
//     // Ok(0)
//     todo!("[blocked] sys_mkdirat")
// }

// // #[rustfmt::skip]
// // fn prepare_io(fd: usize, is_read: bool) -> Result<Arc<File>> {
// //     let process = curr_process();
// //     let inner = process.inner();
// //     if let Some(Some(file)) = inner.fd_table.get(fd) &&
// //         ((is_read && file.readable()) || (!is_read&& file.writable()))
// //     {
// //         let file = Arc::clone(&file.clone());
// //         if file.is_dir() {
// //             return Err(errno::EISDIR);
// //         }
// //         Ok(file)
// //     } else {
// //         Err(errno::EBADF)
// //     }
// // }

/// 从 `fd` 指示的文件中读至多 `len` 字节的数据到用户缓冲区中。成功时返回读入的字节数
///
/// 参数：
/// - `fd` 指定的文件描述符，若无效则返回 `EBADF`，若是目录则返回 `EISDIR`
/// - `buf` 指定用户缓冲区，若无效则返回 `EINVAL`
pub async fn sys_read(fd: usize, buf: UserCheckMut<[u8]>) -> Result {
    if fd == 0 {
        trace!("read stdin, len = {}", buf.len());
    } else {
        debug!("fd = {fd}, len = {}", buf.len());
    }
    // let file = prepare_io(fd, true)?;
    // let nread = file.read(buf);
    // Ok(nread as isize)
    if fd == 0 {
        return Ok(BlockingFuture::new(TtyFuture::new(buf)).await? as isize);
    }
    todo!("[blocked] sys_read full support")
}

/// 向 fd 指示的文件中写入至多 `len` 字节的数据。成功时返回写入的字节数
///
/// 参数：
/// - `fd` 指定的文件描述符，若无效则返回 `EBADF`，若是目录则返回 `EISDIR`
/// - `buf` 指定用户缓冲区，其中存放需要写入的内容，若无效则返回 `EINVAL`
pub async fn sys_write(fd: usize, buf: UserCheck<[u8]>) -> Result {
    let buf = buf.check_slice()?;
    // let file = prepare_io(fd, false)?;
    // let nwrite = file.write(buf);
    // Ok(nwrite as isize)
    if fd == 1 || fd == 2 {
        let s = core::str::from_utf8(&buf).unwrap();
        print!("{s}");
        return Ok(s.len() as isize);
    }
    todo!("[blocked] sys_write full support")
}

// #[repr(C)]
// pub struct IoVec {
//     iov_base: *mut u8,
//     iov_len: usize,
// }

// /// 从 fd 中读入数据，写入多个用户缓冲区中。
// ///
// /// 理论上需要保证原子性，也就是说，即使同时有其他进程（线程）对同一个 fd 进行读操作，
// /// 这一个系统调用也会读入一块连续的区域。目前未实现。
// ///
// /// 参数：
// /// - `fd` 指定文件描述符
// /// - `iovec` 指定 `IoVec` 数组
// /// - `vlen` 指定数组的长度
// pub fn sys_readv(fd: usize, iovec: *const IoVec, vlen: usize) -> Result {
//     // let iovec = unsafe { check_slice(iovec, vlen)? };
//     // let file = prepare_io(fd, true)?;
//     // let mut tot_read = 0;
//     // for iov in iovec {
//     //     let buf = unsafe { check_slice_mut(iov.iov_base, iov.iov_len)? };
//     //     let nread = file.read(buf);
//     //     if nread == 0 {
//     //         break;
//     //     }
//     //     tot_read += nread;
//     // }
//     // Ok(tot_read as isize)
//     todo!("[blocked] sys_readv")
// }

// /// 向 fd 中写入数据，数据来自多个用户缓冲区。
// ///
// /// 理论上需要保证原子性，也就是说，即使同时有其他进程（线程）对同一个 fd 进行写操作，
// /// 这一个系统调用也会写入一块连续的区域。目前未实现。
// ///
// /// 参数：
// /// - `fd` 指定文件描述符
// /// - `iovec` 指定 `IoVec` 数组
// /// - `vlen` 指定数组的长度
// pub fn sys_writev(fd: usize, iovec: *const IoVec, vlen: usize) -> Result {
//     // let iovec = unsafe { check_slice(iovec, vlen)? };
//     // let file = prepare_io(fd, true)?;
//     // let mut total_write = 0;
//     // for iov in iovec {
//     //     let buf = unsafe { check_slice(iov.iov_base, iov.iov_len)? };
//     //     let nwrite = file.write(buf);
//     //     if nwrite == 0 {
//     //         break;
//     //     }
//     //     total_write += nwrite;
//     // }
//     // Ok(total_write as isize)
//     todo!("[blocked] sys_writev")
// }

// // /// 返回一个绝对路径，它指向相对于 `fd` 的名为 `path_name` 的文件
// // fn path_with_fd(fd: usize, path_name: &str) -> Result<String> {
// //     const AT_FDCWD: usize = -100isize as usize;
// //     // 绝对路径则忽视 fd
// //     if path_name.starts_with('/') {
// //         return Ok(path_name.to_string());
// //     }
// //     let process = curr_process();
// //     let inner = process.inner();
// //     if fd == AT_FDCWD {
// //         if path_name == "." {
// //             Ok(inner.cwd.clone())
// //         } else if let Some(path_name) = path_name.strip_prefix("./") {
// //             Ok(format!("{}{}", inner.cwd, path_name))
// //         } else {
// //             Ok(format!("{}{path_name}", inner.cwd))
// //         }
// //     } else if let Some(Some(base)) = inner.fd_table.get(fd) {
// //         let base_path = match &base.entity {
// //             FileEntity::Dir(dir) => dir.path(),
// //             FileEntity::VirtDir(dir) => dir.path(),
// //             _ => return Err(errno::ENOTDIR),
// //         };
// //         Ok(format!("{base_path}/{path_name}"))
// //     } else {
// //         Err(errno::EBADF)
// //     }
// // }

// /// 打开指定的文件。返回非负的文件描述符，这个文件描述符一定是当前进程尚未打开的最小的那个
// ///
// /// 参数：
// /// - `dir_fd` 与 `path_name` 组合形成最终的路径。
// ///     - 若 `path_name` 本身是绝对路径，则忽略。
// ///     - 若 `dir_fd` 等于 `AT_FDCWD`(-100)
// /// - `path_name` 路径，可以是绝对路径 (/xxx/yyy) 或相对路径 (xxx/yyy) 以 `/` 为分隔符
// /// - `flags` 包括文件打开模式、创建标志、状态标志。
// ///     - 创建标志如 `O_CLOEXEC`, `O_CREAT` 等，仅在打开文件时发生作用
// ///     - 状态标志影响后续的 I/O 方式，而且可以动态修改
// /// - `mode` 是用于指定创建新文件时，该文件的 mode。目前应该不会用到
// ///     - 它只会影响未来访问该文件的模式，但这一次打开该文件可以是随意的
// pub fn sys_openat(dir_fd: usize, path_name: *const u8, flags: u32, mut _mode: u32) -> Result {
//     // let file_name = unsafe { check_cstr(path_name)? };

//     // let Some(flags) = OpenFlags::from_bits(flags) else {
//     //     log::error!("open flags: {flags:#b}");
//     //     log::error!("open flags: {:#b}", OpenFlags::O_DIRECTORY.bits());
//     //     return Err(errno::UNSUPPORTED);
//     // };
//     // info!("oepnat {dir_fd}, {file_name}, {flags:?}");
//     // // 不是创建文件（以及临时文件）时，mode 被忽略
//     // if !flags.contains(OpenFlags::O_CREAT) {
//     //     // TODO: 暂时在测试中忽略
//     //     _mode = 0;
//     // }
//     // // TODO: 暂时在测试中忽略
//     // // assert_eq!(mode, 0, "dir_fd: {dir_fd}, flags: {flags:?}");

//     // // 64 位版本应当是保证可以打开大文件的
//     // // TODO: 暂时在测试中忽略
//     // // assert!(flags.contains(OpenFlags::O_LARGEFILE));

//     // // 暂时先不支持这些
//     // if flags.intersects(OpenFlags::O_ASYNC | OpenFlags::O_APPEND | OpenFlags::O_DSYNC) {
//     //     log::error!("todo openflags: {flags:#b}");
//     //     return Err(errno::UNSUPPORTED);
//     // }

//     // let absolute_path = path_with_fd(dir_fd, file_name)?;
//     // let inode = open_file(absolute_path, flags)?;
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // let fd = inner.alloc_fd();
//     // inner.fd_table[fd] = Some(Arc::new(inode));
//     // Ok(fd as isize)
//     todo!("[blocked] sys_openat")
// }

// pub fn sys_close(fd: usize) -> Result {
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // match inner.fd_table.get(fd) {
//     //     Some(Some(_)) => inner.fd_table[fd].take(),
//     //     _ => return Err(errno::EBADF),
//     // };
//     // Ok(0)
//     todo!("[blocked] sys_close")
// }

// /// 创建管道，返回 0
// ///
// /// 参数
// /// - `filedes`: 用于保存 2 个文件描述符。其中，`filedes[0]` 为管道的读出端，`filedes[1]` 为管道的写入端。
// pub fn sys_pipe2(filedes: *mut i32) -> Result {
//     // let filedes = unsafe { check_slice_mut(filedes, 2)? };
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // let (pipe_read, pipe_write) = make_pipe();
//     // let read_fd = inner.alloc_fd();
//     // inner.fd_table[read_fd] = Some(Arc::new(File::new(FileEntity::ReadPipe(pipe_read))));
//     // let write_fd = inner.alloc_fd();
//     // inner.fd_table[write_fd] = Some(Arc::new(File::new(FileEntity::WritePipe(pipe_write))));
//     // info!("read_fd {read_fd}, write_fd {write_fd}");
//     // filedes[0] = read_fd as i32;
//     // filedes[1] = write_fd as i32;
//     // Ok(0)
//     todo!("[blocked] sys_pipe2")
// }

// #[repr(C)]
// pub struct DirEnt64 {
//     /// 索引结点号
//     d_ino: u64,
//     /// 到下一个 dirent 的偏移
//     d_off: i64,
//     /// 当前 dirent 的长度
//     d_reclen: u16,
//     /// 文件类型
//     d_type: u8,
//     /// 文件名
//     d_name: [u8; 0],
// }

// /// 获取目录项信息
// pub fn sys_getdents64(fd: usize, buf: *mut u8, _len: usize) -> Result {
//     // let process = curr_process();
//     // let inner = process.inner();
//     // let Some(Some(_dir)) = inner.fd_table.get(fd) else {
//     //     return Err(errno::EBADF);
//     // };
//     // let mut offset = 0;
//     // let curr_dir_entry = unsafe { check_ptr_mut(buf as *mut DirEnt64)? };
//     // // TODO: d_ino 和 d_type 暂且不管
//     // let entry_len = mem::size_of::<DirEnt64>() + 2;
//     // curr_dir_entry.d_off = entry_len as _; // 2 是 ".\0" 的长度
//     // curr_dir_entry.d_reclen = entry_len as _;
//     // unsafe { *curr_dir_entry.d_name.as_mut_ptr().cast::<[u8; 2]>() = *b".\0" };
//     // offset += entry_len;
//     // // 接下来应该接着遍历目录项，待后续实现

//     // Ok(offset as _)
//     todo!("[blcoked] sys_getdents64")
// }

// /// 操控文件描述符
// ///
// /// 参数：
// /// - `fd` 是指定的文件描述符
// /// - `cmd` 指定需要进行的操作
// /// - `arg` 是该操作可选的参数
// pub fn sys_fcntl64(fd: usize, cmd: usize, arg: usize) -> Result {
//     // const F_DUPFD: usize = 0;
//     // const F_GETFD: usize = 1;
//     // const F_SETFD: usize = 2;
//     // const F_DUPFD_CLOEXEC: usize = 1030;

//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // let Some(Some(file)) = inner.fd_table.get(fd) else {
//     //     return Err(errno::EBADF);
//     // };
//     // match cmd {
//     //     F_DUPFD | F_DUPFD_CLOEXEC => {
//     //         let file = Arc::clone(file);
//     //         let new_fd = inner.alloc_fd_from(arg);
//     //         info!(
//     //             "sys_fcntl64: dup fd {fd}({}) to {new_fd}, with set_close_on_exec = {}",
//     //             file.debug_name(),
//     //             cmd == F_DUPFD_CLOEXEC
//     //         );
//     //         if cmd == F_DUPFD_CLOEXEC {
//     //             file.set_close_on_exec(true);
//     //         }
//     //         inner.fd_table[new_fd] = Some(file);
//     //         Ok(new_fd as isize)
//     //     }
//     //     F_GETFD => {
//     //         info!(
//     //             "sys_fcntl64: get the flag of fd {fd}({})",
//     //             file.debug_name()
//     //         );
//     //         if file.status().contains(OpenFlags::O_CLOEXEC) {
//     //             Ok(1)
//     //         } else {
//     //             Ok(0)
//     //         }
//     //     }
//     //     F_SETFD => {
//     //         info!(
//     //             "sys_fcntl64: set the flag of fd {fd}({}) to {}",
//     //             file.debug_name(),
//     //             arg & 1 != 0
//     //         );
//     //         file.set_close_on_exec(arg & 1 != 0);
//     //         Ok(0)
//     //     }
//     //     _ => {
//     //         log::error!("unsupported cmd: {cmd}, with arg: {arg}");
//     //         Err(errno::EINVAL)
//     //     }
//     // }
//     todo!("[blocked] sys_fcntl64")
// }

// /// 复制文件描述符，复制到当前进程最小可用 fd
// ///
// /// 参数：
// /// - `fd` 是被复制的文件描述符
// pub fn sys_dup(fd: usize) -> Result {
//     // let process = curr_process();
//     // let mut inner = process.inner();
//     // if fd >= inner.fd_table.len() {
//     //     return Err(errno::UNSUPPORTED);
//     // }
//     // if inner.fd_table[fd].is_none() {
//     //     return Err(errno::UNSUPPORTED);
//     // }
//     // let new_fd = inner.alloc_fd();
//     // inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
//     // Ok(new_fd as isize)
//     todo!("[blocked] sys_dup")
// }

// pub fn sys_dup3(old: usize, new: usize) -> Result {
//     let process = curr_process();
//     let mut inner = process.inner();
//     if old >= inner.fd_table.len() {
//         return Err(errno::UNSUPPORTED);
//     }
//     if new >= inner.fd_table.len() {
//         inner.fd_table.resize(new + 1, None);
//     }
//     if inner.fd_table[old].is_none() {
//         return Err(errno::UNSUPPORTED);
//     }
//     inner.fd_table[new] = Some(Arc::clone(inner.fd_table[old].as_ref().unwrap()));
//     Ok(new as isize)
// }

// /// TODO: 写 sys_fstatat 的文档
// pub fn sys_fstatat(dir_fd: usize, file_name: *const u8, statbuf: *mut Stat, flag: usize) -> Result {
//     // TODO: 暂时先不考虑 fstatat 的 flags
//     assert_eq!(flag, 0);
//     let file_name = unsafe { check_cstr(file_name)? };
//     info!("fstatat {dir_fd}, {file_name}");
//     let absolute_path = path_with_fd(dir_fd, file_name)?;
//     info!("absolute path: {absolute_path}");

//     // TODO: 注意，可以尝试用 OpenFlags::O_PATH 打开试试
//     let file = open_file(absolute_path, OpenFlags::empty())?;

//     let statbuf = unsafe { check_ptr_mut(statbuf)? };
//     *statbuf = file.fstat();

//     Ok(0)
// }

// /// FIXME: 由于 mount 未实现，fstat test.txt 也是不成功的
// pub fn sys_fstat(fd: usize, kst: *mut Stat) -> Result {
//     let kst = unsafe { check_ptr_mut(kst)? };
//     let process = curr_process();
//     let inner = process.inner();
//     let Some(Some(file)) = inner.fd_table.get(fd) else {
//         return Err(errno::EBADF);
//     };
//     *kst = file.fstat();

//     Ok(0)
// }

// /// 移除指定文件的链接（可用于删除文件）
// ///
// /// 参数
// ///
// /// TODO: 完善 sys_unlinkat，写文档
// pub fn sys_unlinkat(dirfd: usize, path: *const u8, _flags: u32) -> Result {
//     // TODO: path 相关的操作，不如引入 `unix_path` 这个库来解决？或者自己写个专门的 utils
//     let path = unsafe { check_cstr(path) }?;
//     let path = path_with_fd(dirfd, path)?;
//     let dir_path;
//     let base_name;
//     if path.ends_with('/') {
//         base_name = path[1..path.len() - 1].split('/').next_back().unwrap();
//         dir_path = &path[..path.len() - base_name.len() - 1];
//     } else {
//         base_name = path[1..].split('/').next_back().unwrap();
//         dir_path = &path[..path.len() - base_name.len()];
//     }
//     let Fat32DirOrFile::Dir(dir) = open_fat_entry(
//         dir_path.to_string(),
//         OpenFlags::O_WRONLY | OpenFlags::O_DIRECTORY,
//     )?
//     else {
//         unreachable!("")
//     };
//     dir.remove(base_name)?;
//     Ok(0)
// }

// pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> Result {
//     // FIXME: 尚未实现 sys_linkat
//     Err(errno::UNSUPPORTED)
// }

// /// TODO: sys_umount 完善，写文档
// pub fn sys_umount(_special: *const u8, _flags: i32) -> Result {
//     Ok(0)
// }

// /// TODO: sys_mount 完善，写文档
// pub fn sys_mount(
//     _special: *const u8,
//     _dir: *const u8,
//     _fstype: *const u8,
//     _flags: usize,
//     _data: *const u8,
// ) -> Result {
//     Ok(0)
// }

// /// TODO: sys_chdir 完善，写文档
// pub fn sys_chdir(path: *const u8) -> Result {
//     let path = unsafe { check_cstr(path)? };

//     let mut new_cwd = if !path.starts_with('/') {
//         format!("/{path}")
//     } else {
//         path.to_string()
//     };
//     // 保证目录的格式都是 xxxx/
//     if !new_cwd.ends_with('/') {
//         new_cwd.push('/');
//     }
//     curr_process().inner().cwd = new_cwd;
//     Ok(0)
// }

// /// 获取当前进程当前工作目录的绝对路径。
// ///
// /// 参数：
// /// - `buf` 用于写入路径，以 `\0` 表示字符串结尾
// /// - `size` 如果路径（包括 `\0`）长度大于 `size` 则返回 ERANGE
// pub fn sys_getcwd(buf: *mut u8, size: usize) -> Result {
//     let process = curr_process();
//     let inner = process.inner();
//     let cwd = &inner.cwd;
//     // 包括 '\0'
//     let buf_len = cwd.len() + 1;
//     if buf_len > size {
//         return Err(errno::ERANGE);
//     }
//     {
//         let buf = unsafe { check_slice_mut(buf, buf_len)? };
//         buf[..buf_len - 1].copy_from_slice(cwd.as_bytes());
//         buf[buf_len - 1] = 0;
//     }
//     Ok(buf as isize)
// }

// /// 等待文件描述符上的事件
// ///
// /// TODO: 暂不实现 sys_ppoll
// pub fn sys_ppoll() -> Result {
//     Ok(1)
// }
