use bitflags::bitflags;

use crate::misc::TimeSpec;

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

        /// 是否设置 uid/gid/sticky
        // const S_ISUID = 1 << 11;
        // const S_ISGID = 1 << 10;
        // const S_ISVTX = 1 << 9;
        // TODO: 由于暂时没有权限系统，目前全设为 777
        /// 所有者权限
        const S_IRWXU = Self::S_IRUSR.bits() | Self::S_IWUSR.bits() | Self::S_IXUSR.bits();
        const S_IRUSR = 1 << 8;
        const S_IWUSR = 1 << 7;
        const S_IXUSR = 1 << 6;
        /// 用户组权限
        const S_IRWXG = Self::S_IRGRP.bits() | Self::S_IWGRP.bits() | Self::S_IXGRP.bits();
        const S_IRGRP = 1 << 5;
        const S_IWGRP = 1 << 4;
        const S_IXGRP = 1 << 3;
        /// 其他用户权限
        const S_IRWXO = Self::S_IROTH.bits() | Self::S_IWOTH.bits() | Self::S_IXOTH.bits();
        const S_IROTH = 1 << 2;
        const S_IWOTH = 1 << 1;
        const S_IXOTH = 1 << 0;
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
    /// 但在现代文件系统上可能并不是目录偏移量。因此应用程序应该忽略这个字段，
    /// 不依赖于它
    pub d_off: u64,
    /// 这个 Dirent64 本身的大小
    pub d_reclen: u16,
    /// 文件类型
    pub d_type: u8,
    /// 文件名。实际上是一个 null terminated 的不定长字符串，在 `\0` 之前至多有
    /// `NAME_MAX` 个字符
    pub d_name: [u8; NAME_MAX + 1],
}

/// The stat of a inode
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
    /// 最后一次访问时间 (Access TIME)
    pub st_atime: TimeSpec,
    /// 最后一次修改内容时间 (Modify TIME)
    pub st_mtime: TimeSpec,
    /// 最后一次改变状态时间 (Change TIME)
    pub st_ctime: TimeSpec,
}
