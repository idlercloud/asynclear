use bitflags::bitflags;

use crate::misc::TimeSpec;

// path walk 时，忽略 fd，从当前工作目录开始
pub const AT_FDCWD: usize = -100isize as usize;

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
#[derive(Debug, Default)]
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
