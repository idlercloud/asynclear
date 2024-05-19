use alloc::vec::Vec;
use core::{ops::Deref, str::FromStr};

use cervine::Cow;
use defines::{
    error::{errno, KResult},
    fs::{FstatFlags, IoVec, MountFlags, Stat, UnmountFlags, AT_FDCWD},
};
use triomphe::Arc;

use crate::{
    fs::{
        self, resolve_path_with_dir_fd, DEntry, DirFile, File, FileDescriptor, FileSystemType,
        InodeMode, OpenFlags, PagedFile, VFS,
    },
    hart::local_hart,
    memory::UserCheck,
};

/// 操纵某个特殊文件的底层设备，尤其是字符特殊文件。目前只进行错误检验
///
/// 参数：
/// - `fd` 文件描述符
/// - `request` 请求码，其含义依赖于设备。包含参数是入参还是出参，以及 `argp` 指向的大小
/// - `argp` 一个指针
///
/// 参考：<https://man7.org/linux/man-pages/man2/ioctl.2.html>
pub fn sys_ioctl(fd: usize, request: usize, argp: usize) -> KResult {
    // TODO: [low] 完善 sys_ioctl
    let Some(desc) = local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.fd_table.get(fd).cloned())
    else {
        return Err(errno::EBADF);
    };

    // TODO: [mid] 实现 `sys_ioctl` 的逻辑
    desc.ioctl(request, argp)
}

/// 创建目录。`mode` 含义同 [`sys_openat()`]
pub fn sys_mkdirat(dir_fd: usize, path: UserCheck<u8>, _mode: usize) -> KResult {
    // TODO: [low] 暂时未支持 mode
    let path = path.check_cstr()?;
    let p2i = fs::resolve_path_with_dir_fd(dir_fd, &path)?;
    p2i.dir.mkdir(p2i.last_component)?;
    Ok(0)
}

fn prepare_io<const READ: bool>(fd: usize) -> KResult<FileDescriptor> {
    let process = local_hart().curr_process();
    let inner = process.lock_inner();
    let file = inner.fd_table.get(fd).ok_or(errno::EBADF)?;
    if (READ && file.readable()) || (!READ && file.writable()) {
        Ok(file.clone())
    } else {
        Err(errno::EBADF)
    }
}

/// 从 `fd` 指示的文件中读至多 `len`
/// 字节的数据到用户缓冲区中。成功时返回读入的字节数
///
/// 参数：
/// - `fd` 指定的文件描述符，若无效则返回 `EBADF`，若是目录则返回 `EISDIR`
/// - `buf` 指定用户缓冲区，若无效则返回 `EINVAL`
pub async fn sys_read(fd: usize, buf: UserCheck<[u8]>) -> KResult {
    if fd == 0 {
        trace!("read stdin, len = {}", buf.len());
    } else {
        debug!("read fd = {fd}, len = {}", buf.len());
    }

    let file = prepare_io::<true>(fd)?;
    let nread = file.read(buf).await?;
    Ok(nread as isize)
}

/// 向 fd 指示的文件中写入至多 `len` 字节的数据。成功时返回写入的字节数
///
/// 参数：
/// - `fd` 指定的文件描述符，若无效则返回 `EBADF`，若是目录则返回 `EISDIR`
/// - `buf` 指定用户缓冲区，其中存放需要写入的内容，若无效则返回 `EINVAL`
pub async fn sys_write(fd: usize, buf: UserCheck<[u8]>) -> KResult {
    if fd == 1 || fd == 2 {
        trace!("write stdout/stderr, len = {}", buf.len());
    } else {
        debug!("write fd = {fd}, len = {}", buf.len());
    }

    let file = prepare_io::<false>(fd)?;
    let nwrite = file.write(buf).await?;
    Ok(nwrite as isize)
}

/// 从 fd 中读入数据，写入多个用户缓冲区中。
///
/// 理论上需要保证原子性，也就是说，即使同时有其他进程（线程）对同一个 fd 进行读操作，这一个系统调用也会读入一块连续的区域
///
/// 参数：
/// - `fd` 指定文件描述符
/// - `iovec` 指定 `IoVec` 数组
pub async fn sys_readv(fd: usize, iovec: UserCheck<[IoVec]>) -> KResult {
    if fd == 1 || fd == 2 {
        trace!("writev stdout/stderr");
    } else {
        debug!("writev fd = {fd}");
    }
    let iovec = iovec.check_slice()?;
    let file = prepare_io::<true>(fd)?;
    let mut tot_read = 0;
    // TODO: [mid] 改变 `sys_readv` 的实现方式使其满足原子性
    // NOTE: `IoVec` 带裸指针所以不 Send 也不 Sync，因此用下标而非迭代器来绕一下
    let mut iov_index = 0;
    while let Some(iov) = iovec.read_at(iov_index) {
        iov_index += 1;
        let buf = UserCheck::new_slice(iov.iov_base, iov.iov_len);
        let nread = file.read(buf).await?;
        if nread == 0 {
            break;
        }
        tot_read += nread;
    }
    Ok(tot_read as isize)
}

/// 向 fd 中写入数据，数据来自多个用户缓冲区。
///
/// 理论上需要保证原子性，也就是说，即使同时有其他进程（线程）对同一个 fd 进行写操作，这一个系统调用也会写入一块连续的区域。
///
/// 参数：
/// - `fd` 指定文件描述符
/// - `iovec` 指定 `IoVec` 数组
/// - `vlen` 指定数组的长度
pub async fn sys_writev(fd: usize, iovec: UserCheck<[IoVec]>) -> KResult {
    if fd == 0 {
        trace!("writev stdout");
    } else {
        debug!("writev fd = {fd}");
    }
    let iovec = iovec.check_slice()?;
    let file = prepare_io::<false>(fd)?;
    let mut total_write = 0;
    let mut iov_index = 0;
    // TODO: [mid] 改变 `sys_writev` 的实现方式使其满足原子性
    // NOTE: `IoVec` 带裸指针所以不 Send 也不 Sync，因此用下标而非迭代器来绕一下
    while let Some(iov) = iovec.read_at(iov_index) {
        iov_index += 1;
        let buf = UserCheck::new_slice(iov.iov_base, iov.iov_len);
        let nwrite = file.write(buf).await?;
        if nwrite == 0 {
            break;
        }
        total_write += nwrite;
    }
    Ok(total_write as isize)
}

/// 打开指定的文件。返回非负的文件描述符，这个文件描述符一定是当前进程尚未打开的最小的那个
///
/// 参数：
/// - `dir_fd` 与 `path` 组合形成最终的路径。
///     - 若 `path` 本身是绝对路径，则忽略。
///     - 若 `dir_fd` 等于 `AT_FDCWD`(-100)，则以 cwd 为起点计算相对路径
/// - `path` 路径，可以是绝对路径或相对路径，以 `/` 为分隔符
/// - `flags` 包括文件打开模式、创建标志、状态标志。
///     - 创建标志如 `CLOEXEC`, `CREAT` 等，仅在打开文件时发生作用
///     - 状态标志影响后续的 I/O 方式，而且可以动态修改
/// - `mode` 是用于指定创建新文件时，该文件的 mode。目前应该不会用到
///     - 它只会影响未来访问该文件的模式，但这一次打开该文件可以是随意的
pub async fn sys_openat(dir_fd: usize, path: UserCheck<u8>, flags: u32, mut _mode: u32) -> KResult {
    let path = path.check_cstr()?;

    let Some(flags) = OpenFlags::from_bits(flags) else {
        todo!("[low] unsupported OpenFlags: {flags:#b}");
    };
    info!("oepnat flags {flags:?}");

    // TODO: [low] OpenFlags::DIRECT 目前是被忽略的
    // TODO: [low] 暂时未支持 mode
    // 不是创建文件（以及临时文件）时，mode 被忽略
    if !flags.contains(OpenFlags::CREATE) {
        _mode = 0;
    }

    // 64 位版本应当是保证可以打开大文件的
    // TODO: [low] 暂时在测试中忽略 `OpenFlags::LARGEFILE` 的检查
    // assert!(flags.contains(OpenFlags::LARGEFILE));

    // 暂时先不支持这些
    if flags.intersects(OpenFlags::ASYNC | OpenFlags::DSYNC) {
        todo!("[low] unsupported openflags: {flags:#b}");
    }

    let p2i = fs::resolve_path_with_dir_fd(dir_fd, &path)?;
    // TODO: [low] 其实可以做一个 `CompactCowString` 避免不必要的拷贝
    let new_file = if let Some(final_dentry) = p2i.dir.lookup(Cow::Borrowed(&p2i.last_component)) {
        // 指定了必须要创建文件，但该文件已存在
        if flags.contains(OpenFlags::CREATE | OpenFlags::EXCL) {
            return Err(errno::EEXIST);
        }

        match final_dentry {
            DEntry::Dir(dir) => {
                // 路径名指向一个目录，但是需要写入
                if flags.intersects(OpenFlags::WRONLY | OpenFlags::RDWR) {
                    return Err(errno::EISDIR);
                };
                File::Dir(Arc::new(DirFile::new(dir)))
            }
            DEntry::Paged(paged) => {
                if flags.contains(OpenFlags::DIRECTORY) {
                    return Err(errno::ENOTDIR);
                }
                File::Paged(Arc::new(PagedFile::new(paged)))
            }
        }
    } else {
        if !flags.contains(OpenFlags::CREATE) {
            // 找不到该文件，而且又没有指定 `OpenFlags::CREATE`
            return Err(errno::ENOENT);
        }

        debug!(
            "create {} under {}",
            p2i.last_component,
            p2i.dir.inode().meta().name()
        );
        let dentry = p2i.dir.mknod(p2i.last_component, InodeMode::Regular)?;
        File::Paged(Arc::new(PagedFile::new(dentry)))
    };

    let ret_fd = local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.fd_table.add(FileDescriptor::new(new_file, flags)));
    Ok(ret_fd as isize)
}

pub fn sys_close(fd: usize) -> KResult {
    let process = local_hart().curr_process();
    if process
        .lock_inner_with(|inner| inner.fd_table.remove(fd))
        .is_none()
    {
        return Err(errno::EBADF);
    }

    // TODO: [low] 还要释放相关的记录锁
    // TODO: [mid] 如果文件被 `unlink()` 了且当前 fd 是最后一个引用该文件的，则要删除该文件

    Ok(0)
}

/// 创建管道，返回 0
///
/// 参数
/// - `filedes`: 用于保存 2 个文件描述符。其中，`filedes[0]` 为管道的读出端，`filedes[1]` 为管道的写入端。
/// - `flags`: 同 [`sys_openat()`] 的 [`OpenFlags`]，只有某些位有用
pub fn sys_pipe2(pipefd: UserCheck<[i32; 2]>, flags: u32) -> KResult {
    let pipefd = unsafe { pipefd.check_ptr_mut()? };
    let Some(flags) = OpenFlags::from_bits(flags) else {
        todo!("[low] unsupported OpenFlags: {flags:#b}");
    };
    let (read_end, write_end) = fs::make_pipe();
    let read_end = FileDescriptor::new(File::Pipe(read_end), flags.with_read_only());
    let write_end = FileDescriptor::new(File::Pipe(write_end), flags.with_write_only());
    let fds = local_hart()
        .curr_process()
        .lock_inner_with(|inner| (inner.fd_table.add(read_end), inner.fd_table.add(write_end)));
    pipefd.write([fds.0 as i32, fds.1 as i32]);

    Ok(0)
}

/// 获取目录项信息
pub fn sys_getdents64(fd: usize, buf: UserCheck<[u8]>) -> KResult {
    let process = local_hart().curr_process();
    let Some(File::Dir(dir)) =
        process.lock_inner_with(|inner| inner.fd_table.get(fd).map(Deref::deref).cloned())
    else {
        return Err(errno::EBADF);
    };
    let buf = unsafe { buf.check_slice_mut()? };
    let ret = dir.getdirents(buf.out())?;
    Ok(ret as isize)
}

/// 操控文件描述符
///
/// 参数：
/// - `fd` 是指定的文件描述符
/// - `cmd` 指定需要进行的操作
/// - `arg` 是该操作可选的参数
pub fn sys_fcntl64(fd: usize, cmd: usize, arg: usize) -> KResult {
    // 未说明返回值的命令成功时都返回 0
    /// 复制该 fd 到大于等于 `arg` 的第一个可用 fd。成功后返回新的 fd
    const F_DUPFD: usize = 0;
    /// 同 `F_DUPFD`，不过为新 fd 设置 `CLOEXEC` 标志
    const F_DUPFD_CLOEXEC: usize = 1030;
    // 下面两个是文件描述符标志操作。目前只有一个 `FD_CLOEXEC` 标志
    /// 返回文件描述符标志，`arg` 将被忽略
    const F_GETFD: usize = 1;
    /// 将文件描述符标志设置为 `arg` 指定的值
    const F_SETFD: usize = 2;
    // 下面两个是文件状态标志操作
    // /// 返回文件访问模式和文件状态标志，`arg` 将被忽略
    // const F_GETFL: i32 = 3;
    // /// 将文件状态标志设置为 `arg` 指定的值。
    // ///
    // /// 在 Linux 上，此命令只能更改 `O_APPEND`、`O_ASYNC`、`O_DIRECT`、`O_NOATIME`` 和 `O_NONBLOCK` 标志。
    // /// 无法更改 `O_DSYNC` 和 `O_SYNC` 标志。
    // const F_SETFL: i32 = 4;

    debug!("fd: {fd}, cmd: {cmd:#x}, arg: {arg:#x}");

    let process = local_hart().curr_process();
    let mut inner = process.lock_inner();

    match cmd {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            let mut desc = inner.fd_table.get(fd).ok_or(errno::EBADF)?.clone();
            if cmd == F_DUPFD_CLOEXEC {
                desc.set_close_on_exec(true);
            }
            let new_fd = inner.fd_table.add_from(desc, arg);
            debug!(
                "dup fd {fd}({}) to {new_fd}, with close_on_exec = {}",
                inner.fd_table.get(new_fd).unwrap().meta().name(),
                cmd == F_DUPFD_CLOEXEC
            );
            Ok(new_fd as isize)
        }
        F_GETFD => {
            let desc = inner.fd_table.get(fd).ok_or(errno::EBADF)?;
            debug!("get the CLOEXEC flag of fd {fd}({})", desc.meta().name());
            if desc.flags().contains(OpenFlags::CLOEXEC) {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        F_SETFD => {
            let desc = inner.fd_table.get_mut(fd).ok_or(errno::EBADF)?;
            debug!(
                "set the CLOEXEC flag of fd {fd}({}) to {}",
                desc.meta().name(),
                arg & 1 != 0
            );
            desc.set_close_on_exec(arg & 1 != 0);
            Ok(0)
        }
        _ => {
            error!("unsupported cmd: {cmd}, with arg: {arg}");
            Err(errno::EINVAL)
        }
    }
}

// /// 复制文件描述符，复制到
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
//     // inner.fd_table[new_fd] =
// Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));     // Ok(new_fd as
// isize)     todo!("[blocked] sys_dup")
// }

/// 复制文件描述符 `old_fd` 到当前进程最小可用 fd
pub fn sys_dup(old_fd: usize) -> KResult {
    let process = local_hart().curr_process();
    let mut inner = process.lock_inner();
    let Some(new_desc) = inner.fd_table.get(old_fd).cloned() else {
        return Err(errno::EBADF);
    };
    let new_fd = inner.fd_table.add(new_desc);
    Ok(new_fd as isize)
}

/// 复制文件描述符 `old_fd` 到指定描述符 `new_fd`
///
/// 如果 `new_fd` 已经被打开，则它被原子地关闭再复用
///
/// 如果 `flags` 包括 CLOEXEC 位，则新描述符会被设置该标志
///
/// 参数：
/// - `old_fd` 被复制的描述符
/// - `new_fd` 要复制到的描述符
/// - `flags` [`OpenFlags`]，实际应该只用到 [`OpenFlags::CLOEXEC`]
pub fn sys_dup3(old_fd: usize, new_fd: usize, flags: u32) -> KResult {
    let Some(flags) = OpenFlags::from_bits(flags) else {
        todo!("[low] unsupported OpenFlags: {flags:#b}");
    };
    let process = local_hart().curr_process();
    let mut inner = process.lock_inner();
    let Some(desc) = inner.fd_table.get(old_fd) else {
        return Err(errno::EBADF);
    };
    if old_fd == new_fd {
        return Err(errno::EINVAL);
    }
    let mut new_desc = desc.clone();
    if flags.contains(OpenFlags::CLOEXEC) {
        new_desc.set_close_on_exec(true);
    }
    inner.fd_table.insert(new_fd, new_desc);
    Ok(new_fd as isize)
}

/// 获取一个文件的信息
///
/// 参数：
/// - `dir_fd` 开始搜索文件的目录，参考 [`sys_openat()`]
/// - `path` 相对路径或绝对路径
/// - `statbuf` 文件信息写入的目的地
/// - `flags` fstat 的一些 flags
pub fn sys_newfstatat(
    dir_fd: usize,
    path: UserCheck<u8>,
    statbuf: UserCheck<Stat>,
    flags: usize,
) -> KResult {
    let flags = FstatFlags::from_bits(u32::try_from(flags).map_err(|_e| errno::EINVAL)?)
        .ok_or(errno::EINVAL)?;
    let file_name = path.check_cstr()?;
    if file_name.is_empty() && !flags.contains(FstatFlags::AT_EMPTY_PATH) {
        return Err(errno::ENOENT);
    }
    let statbuf = unsafe { statbuf.check_ptr_mut()? };
    let stat = if file_name.is_empty() {
        let process = local_hart().curr_process();
        let inner = process.lock_inner();
        let file = inner.fd_table.get(dir_fd).ok_or(errno::EBADF)?;
        fs::stat_from_meta(file.meta())
    } else {
        let p2i = fs::resolve_path_with_dir_fd(dir_fd, &file_name)?;
        let dentry = p2i
            .dir
            .lookup(Cow::Owned(p2i.last_component))
            .ok_or(errno::ENOENT)?;
        fs::stat_from_meta(dentry.meta())
    };
    statbuf.write(stat);

    Ok(0)
}

pub fn sys_newfstat(fd: usize, statbuf: UserCheck<Stat>) -> KResult {
    let process = local_hart().curr_process();
    let stat = process.lock_inner_with(|inner| {
        let file = inner.fd_table.get(fd).ok_or(errno::EBADF)?;
        Ok(fs::stat_from_meta(file.meta()))
    })?;
    let statbuf = unsafe { statbuf.check_ptr_mut()? };
    statbuf.write(stat);

    Ok(0)
}

/// 移除指定文件的链接（可用于删除文件）。成功时返回 0
///
/// 参数：
/// - `dir_fd` 要删除的链接所在的目录
/// - `path` 要删除的链接的名字
/// - `flags` 可设置为 0 或 `AT_REMOVEDIR`
pub fn sys_unlinkat(dir_fd: usize, path: UserCheck<u8>, flags: u32) -> KResult {
    let path = path.check_cstr()?;
    let Some(flags) = FstatFlags::from_bits(flags) else {
        todo!("[low] unsupported OpenFlags: {flags:#b}");
    };
    info!("flags {flags:?}");
    let p2i = resolve_path_with_dir_fd(dir_fd, &path)?;
    let dentry = p2i
        .dir
        .lookup(Cow::Owned(p2i.last_component))
        .ok_or(errno::ENOENT)?;
    if flags.contains(FstatFlags::AT_REMOVEDIR) {
        todo!("[mid] impl rmdir");
    } else {
        let DEntry::Paged(paged) = dentry else {
            return Err(errno::EISDIR);
        };
        paged.parent().unlink(paged.inode().meta().name())?;
    }
    Ok(0)
}

/// 创建文件的（硬）链接，成功返回 0
///
/// 参数：
/// - `old_dir_fd` 原来的文件所在目录的文件描述符
/// - `old_path` 文件原来的名字
/// - `new_dir_fd` 新文件名所在的目录
/// - `new_path` 文件的新名字
/// - `flags` 可包含 `AT_SYMLINK_FOLLOW` 和 `AT_EMPTY_PATH`
#[allow(unused)]
pub fn sys_linkat(
    old_dir_fd: usize,
    old_path: UserCheck<u8>,
    new_dir_fd: usize,
    new_path: UserCheck<u8>,
    flags: u32,
) -> KResult {
    // 在 2.6.18 内核之前，应置为 0
    if flags != 0 {
        return Err(errno::EINVAL);
    }
    todo!("[low] impl sys_linkat")
}

// TODO: [low] 完善 mount 和 umount

/// 卸载安装在 `target` 上的文件系统
pub fn sys_umount(target: UserCheck<u8>, flags: u32) -> KResult {
    let Some(flags) = UnmountFlags::from_bits(flags) else {
        todo!("[low] unsupported MountFlags: {flags:#b}");
    };
    let target = target.check_cstr()?;
    VFS.unmount(&target, flags)?;
    Ok(0)
}

/// 将 `source` 指定的文件系统（通常是设备的路径名，但也可以是目录或文件的路径名，或者虚拟字符串）附加到路径名指定的位置（目录或文件）在目标中。
pub fn sys_mount(
    source: UserCheck<u8>,
    target: UserCheck<u8>,
    fs_type: UserCheck<u8>,
    flags: u32,
    data: UserCheck<u8>,
) -> KResult {
    let source = source.check_cstr()?;
    let target = target.check_cstr()?;
    let fs_type = FileSystemType::from_str(&fs_type.check_cstr()?)?;
    let Some(flags) = MountFlags::from_bits(flags) else {
        todo!("[low] unsupported MountFlags: {flags:#b}");
    };
    if !data.is_null() {
        let _data = data.check_cstr()?;
    }

    VFS.mount(&target, &source, fs_type, flags)?;
    Ok(0)
}

/// 将调用进程的当前工作目录更改为 `path` 中指定的目录
pub fn sys_chdir(path: UserCheck<u8>) -> KResult {
    let path = path.check_cstr()?;
    let p2i = fs::resolve_path_with_dir_fd(AT_FDCWD, &path)?;
    let DEntry::Dir(dir) = p2i
        .dir
        .lookup(Cow::Owned(p2i.last_component))
        .ok_or(errno::ENOENT)?
    else {
        return Err(errno::ENOTDIR);
    };
    local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.cwd = dir);
    Ok(0)
}

/// 获取当前进程当前工作目录的绝对路径
///
/// 参数：
/// - `buf` 用于写入路径，以 `\0` 表示字符串结尾
/// - `size` 如果路径（包括 `\0`）长度大于 `size` 则返回 ERANGE
pub fn sys_getcwd(buf: UserCheck<[u8]>) -> KResult {
    let ret = buf.addr() as isize;
    let cwd = local_hart()
        .curr_process()
        .lock_inner_with(|inner| Arc::clone(&inner.cwd));
    let mut dirs = Vec::new();
    let mut dir = &cwd;
    // 根目录 `/` 和 `\0`
    let mut path_len = 2;
    loop {
        let Some(parent) = dir.parent() else {
            break;
        };
        path_len += dir.inode().meta().name().len();
        dirs.push(dir);
        dir = parent;
    }

    path_len += dirs.len().saturating_sub(1);

    if path_len > buf.len() {
        return Err(errno::ERANGE);
    }
    let buf = unsafe { buf.check_slice_mut()? };
    let mut buf = buf.out();

    buf.reborrow().get_out(0).unwrap().write(b'/');
    let mut curr = 1;
    for name in dirs
        .into_iter()
        .rev()
        .map(|dir| dir.inode().meta().name().as_bytes())
        .intersperse(b"/")
    {
        buf.reborrow()
            .get_out(curr..curr + name.len())
            .unwrap()
            .copy_from_slice(name);
        curr += name.len();
    }

    Ok(ret)
}

// /// 等待文件描述符上的事件
// ///
// /// TODO: 暂不实现 sys_ppoll
// pub fn sys_ppoll() -> Result {
//     Ok(1)
// }
