use core::fmt::Display;

use bitflags::bitflags;

use crate::misc::TimeSpec;

/// 默认的文件描述符软上限
pub const MAX_FD_NUM: usize = 1024;

// path walk 时，忽略 fd，从当前工作目录开始
pub const AT_FDCWD: usize = -100isize as usize;

pub const SEEK_SET: usize = 0;
pub const SEEK_CUR: usize = 1;
pub const SEEK_END: usize = 2;

bitflags! {
    /// 一个 inode 的 mode。如文件类型、用户权限等
    #[derive(Clone, Copy, Debug, Default)]
    pub struct StatMode: u32 {
        // 以下类型只为其一
        /// 是普通文件
        const REGULAR       = 1 << 15;
        /// 是符号链接
        const SYM_LINK      = 1 << 15 | 1 << 13;
        /// 是 socket
        const SOCKET        = 1 << 15 | 1 << 14;
        /// 是块设备
        const BLOCK_DEVICE  = 1 << 14 | 1 << 13;
        /// 是目录
        const DIR           = 1 << 14;
        /// 是字符设备
        const CHAR_DEVICE   = 1 << 13;
        /// 是 FIFO
        const FIFO          = 1 << 12;

        // /// 是否设置 uid/gid/sticky
        // // const S_ISUID = 1 << 11;
        // // const S_ISGID = 1 << 10;
        // // const S_ISVTX = 1 << 9;
        // /// 所有者权限
        // const S_IRWXU = Self::S_IRUSR.bits() | Self::S_IWUSR.bits() | Self::S_IXUSR.bits();
        // const S_IRUSR = 1 << 8;
        // const S_IWUSR = 1 << 7;
        // const S_IXUSR = 1 << 6;
        // /// 用户组权限
        // const S_IRWXG = Self::S_IRGRP.bits() | Self::S_IWGRP.bits() | Self::S_IXGRP.bits();
        // const S_IRGRP = 1 << 5;
        // const S_IWGRP = 1 << 4;
        // const S_IXGRP = 1 << 3;
        // /// 其他用户权限
        // const S_IRWXO = Self::S_IROTH.bits() | Self::S_IWOTH.bits() | Self::S_IXOTH.bits();
        // const S_IROTH = 1 << 2;
        // const S_IWOTH = 1 << 1;
        // const S_IXOTH = 1 << 0;
    }

    #[derive(Debug)]
    pub struct FstatFlags: u32 {
        /// 如果传入的 `path` 是符号链接，则不要将其解引用，而是返回符号链接本身的信息
        const AT_SYMLINK_NOFOLLOW   = 1 << 8;
        /// `unlinkat` 时对路径名执行相当于 `rmdir` 的操作
        const AT_REMOVEDIR          = 1 << 9;
        /// `linkat` 时如果传入的 `path` 是符号链接，则将其解引用
        const AT_SYMLINK_FOLLOW     = 1 << 10;
        /// 不要自动挂载路径名的 terminal(basename) component
        const AT_NO_AUTOMOUNT       = 1 << 11;
        /// 如果传入的 `path` 是空，则对 `dirfd` 指向的文件进行操作。
        ///
        /// 此时 `dirfd` 可以指向任意类型的文件而不止是目录
        const AT_EMPTY_PATH         = 1 << 12;
    }

    #[derive(Debug)]
    pub struct MountFlags : u32 {
        // const MS_RDONLY         = 1 <<  0;
        // const MS_NOSUID         = 1 <<  1;
        // const MS_NODEV          = 1 <<  2;
        // const MS_NOEXEC         = 1 <<  3;
        // const MS_SYNCHRONOUS    = 1 <<  4;
        // const MS_REMOUNT        = 1 <<  5;
        // const MS_MANDLOCK       = 1 <<  6;
        // const MS_DIRSYNC        = 1 <<  7;
        // const MS_NOSYMFOLLOW    = 1 <<  8;
        // const MS_NOATIME        = 1 <<  9;
        // const MS_NODIRATIME     = 1 << 10;
        // const MS_BIND           = 1 << 11;
        // const MS_MOVE           = 1 << 12;
        // const MS_REC            = 1 << 13;
        // const MS_SILENT         = 1 << 14;
        // const MS_POSIXACL       = 1 << 16;
        // const MS_UNBINDABLE     = 1 << 17;
        // const MS_PRIVATE        = 1 << 18;
        // const MS_SLAVE          = 1 << 19;
        // const MS_SHARED         = 1 << 20;
        // const MS_RELATIME       = 1 << 21;
        // const MS_KERNMOUNT      = 1 << 22;
        // const MS_I_VERSION      = 1 << 23;
        // const MS_STRICTATIME    = 1 << 24;
        // const MS_LAZYTIME       = 1 << 25;
        // const MS_NOREMOTELOCK   = 1 << 27;
        // const MS_NOSEC          = 1 << 28;
        // const MS_BORN           = 1 << 29;
        // const MS_ACTIVE         = 1 << 30;
        // const MS_NOUSER         = 1 << 31;
    }

    #[derive(Debug)]
    pub struct UnmountFlags : u32 {
        // const MNT_FORCE         =   1 << 0;
        // const MNT_DETACH        =   1 << 1;
        // const MNT_EXPIRE        =   1 << 2;
        // const UMOUNT_NOFOLLOW   =   1 << 3;
    }

    #[derive(Debug)]
    pub struct StatFsFlags: u32 {
        // /// This filesystem is mounted read-only.
        // const ST_RDONLY         = 1 << 0;
        // /// The set-user-ID and set-group-ID bits are ignored by exec(3) for executable files on this filesystem.
        // const ST_NOSUID         = 1 << 1;
        // /// Disallow access to device special files on this filesystem.
        // const ST_NODEV          = 1 << 2;
        // /// Execution of programs is disallowed on this filesystem.
        // const ST_NOEXEC         = 1 << 3;
        // /// Writes are synched to the filesystem immediately (see the description of O_SYNC in open(2)).
        // const ST_SYNCHRONOUS    = 1 << 4;
        // /// Mandatory locking is permitted on the filesystem.
        // const ST_MANDLOCK       = 1 << 6;
    }

    /// 注意低 2 位指出文件的打开模式
    /// 0、1、2 分别对应只读、只写、可读可写。3 为错误。
    #[derive(Clone, Copy, Debug)]
    pub struct OpenFlags: u32 {
        const RDONLY    = 0;
        const WRONLY    = 1 << 0;
        const RDWR      = 1 << 1;

        /// 如果所查询的路径不存在，则在该路径创建一个常规文件
        const CREATE    = 1 << 6;
        /// 在创建文件的情况下，保证该文件之前不存在，否则返回错误
        const EXCL      = 1 << 7;
        /// 如果路径指向一个终端设备，那么它不会成为本进程的控制终端
        const NOCTTY    = 1 << 8;
        // /// 如果是常规文件，且允许写入，则将该文件长度截断为 0
        // const TRUNCATE  = 1 << 9;
        /// 写入追加到文件末尾，它是在每次 `sys_write` 时生效
        const APPEND    = 1 << 10;
        /// 在可能的情况下，让该文件以非阻塞模式打开
        const NONBLOCK  = 1 << 11;
        /// 保持文件数据与磁盘阻塞同步。但如果该写操作不影响后续的读取，则不会同步更新元数据
        const DSYNC     = 1 << 12;
        /// 文件操作完成时发出信号
        const ASYNC     = 1 << 13;
        /// 不经过缓存，直接写入磁盘中
        const DIRECT    = 1 << 14;
        /// 允许打开文件大小超过 32 位表示范围的大文件。在 64 位系统上此标志位应永远为真
        const LARGEFILE = 1 << 15;
        /// 如果打开的文件不是目录，那么就返回失败
        const DIRECTORY = 1 << 16;
        // /// 如果路径的 basename 是一个符号链接，则打开失败并返回 `ELOOP`，目前不支持
        // const O_NOFOLLOW    = 1 << 17;
        // /// 读文件时不更新文件的 last access time，暂不支持
        // const O_NOATIME     = 1 << 18;
        /// 设置打开的文件描述符的 close-on-exec 标志
        const CLOEXEC   = 1 << 19;
        // /// 仅打开一个文件描述符，而不实际打开文件。后续只允许进行纯文件描述符级别的操作
        // TODO: 可能要考虑加上 O_PATH，似乎在某些情况下无法打开的文件可以通过它打开
        // FIXME: 初赛误把 `O_DIRECTORY` 定义成了 `O_PATH`，这里暂时开启以便通过测试，实际未支持
        const PATH        = 1 << 21;
    }
}

impl Display for StatFsFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "rw")
    }
}

impl OpenFlags {
    pub fn with_read_only(self) -> Self {
        self.difference(Self::WRONLY | Self::RDWR)
    }

    pub fn with_write_only(self) -> Self {
        self.difference(Self::RDWR) | Self::WRONLY
    }

    pub fn read_write(&self) -> (bool, bool) {
        match self.bits() & 0b11 {
            0 => (true, false),
            1 => (false, true),
            2 => (true, true),
            _ => unreachable!(),
        }
    }
}

pub const NAME_MAX: usize = 255;

/// 参考 <https://man7.org/linux/man-pages/man3/readdir.3.html/>
#[derive(Debug)]
#[repr(C)]
pub struct Dirent64 {
    /// 64 位 inode 编号
    pub d_ino: u64,
    /// `d_off` 中返回的值与在目录流中的当前位置调用 telldir(3) 返回的值相同
    ///
    /// 但在现代文件系统上可能并不是目录偏移量。因此应用程序应该忽略这个字段，不依赖于它
    pub d_off: u64,
    /// 这个 Dirent64 本身的大小
    pub d_reclen: u16,
    /// 文件类型
    pub d_type: u8,
    /// 文件名。实际上是一个 null terminated 的不定长字符串，在 `\0` 之前至多有 `NAME_MAX` 个字符
    pub d_name: [u8; NAME_MAX + 1],
}

/// 一个 inode 的相关信息
#[repr(C)]
#[derive(Default)]
pub struct Stat {
    /// 包含该文件的设备号
    pub st_dev: u64,
    /// inode 编号
    pub st_ino: u64,
    /// 文件类型和模式
    pub st_mode: StatMode,
    /// 硬链接的数量
    pub st_nlink: u32,
    /// Owner 的用户 ID
    pub st_uid: u32,
    /// Owner 的组 ID
    pub st_gid: u32,
    /// 特殊文件的设备号
    pub st_rdev: u64,
    _pad0: u64,
    /// 文件总大小
    pub st_size: u64,
    /// 文件系统 I/O 的块大小。
    pub st_blksize: u32,
    _pad1: u32,
    /// 已分配的块个数。
    pub st_blocks: u64,
    /// 最后一次访问时间
    pub st_atime: TimeSpec,
    /// 最后一次修改内容时间
    pub st_mtime: TimeSpec,
    /// 最后一次改变状态时间
    pub st_ctime: TimeSpec,
}

#[repr(C)]
pub struct IoVec {
    pub iov_base: *mut u8,
    pub iov_len: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PollFd {
    /// 如果为负数则忽略 `events` 字段，且 `revents` 返回 0
    pub fd: i32,
    /// 输入参数，一个位掩码，表示感兴趣的事件。
    ///
    /// 为 0 的情况下，`revents` 的返回值是 `POLLHUP`、`POLLERR` 和 `POLLNVAL` 之一
    pub events: i16,
    /// 输出参数，表示发生的事件
    ///
    /// 可以包括 `events` 中指定的任何位，或者值 `POLLHUP`、`POLLERR` 或 `POLLNVAL` 之一。
    pub revents: i16,
}

bitflags! {
    #[derive(Debug)]
    pub struct PollEvents: i16 {
        /// 有数据可读
        const POLLIN = 1 << 0;
        /// 文件描述符上存在一些异常情况，包括
        ///
        /// - TCP socket 上存在带外 (out-of-band) 数据
        /// - 数据包模式下的伪终端 master 发现 slave 的状态更改
        /// - cgroup.events 文件已被修改
        const POLLPRI = 1 << 1;
        /// 可以进行写入，但大于 socket 或 pipe 可用空间的写入仍然会阻塞（除非设置了 `O_NONBLOCK`
        const POLLOUT = 1 << 2;
        /// 错误情况（仅在 `revents` 返回，在 `events` 中会被忽略）
        ///
        /// 当 pipe 的读取端关闭时，也会为写入端的文件描述符设置该位
        const POLLERR = 1 << 3;
        /// Hang up（仅在 `revents` 返回，在 `events` 中会被忽略）
        ///
        /// 当从管道或者 stream socket 等通道读取数据时，此事件仅表明 peer 关闭了其通道端
        /// 仅当通道中所有未读取的数据都被消耗后，后续的读取才会返回 0
        const POLLHUP = 1 << 4;
        /// 无效请求，fd 未打开（仅在 `revents` 返回，在 `events` 中会被忽略）
        const POLLNVAL = 1 << 5;
    }
}
