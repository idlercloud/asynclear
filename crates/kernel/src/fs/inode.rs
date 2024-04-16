use bitflags::bitflags;

// pub struct Inode {
//     mode: StatMode,
// }

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    #[derive(Clone, Copy, Debug, Default)]
    pub struct StatMode: u32 {
        // 以下类型只为其一
        /// 是普通文件
        const S_IFREG  = 1 << 15;
        /// 是符号链接
        const S_IFLNK  = 1 << 15 | 1 << 13;
        /// 是 socket
        const S_IFSOCK = 1 << 15 | 1 << 14;
        /// 是块设备
        const S_IFBLK  = 1 << 14 | 1 << 13;
        /// 是目录
        const S_IFDIR  = 1 << 14;
        /// 是字符设备
        const S_IFCHR  = 1 << 13;
        /// 是 FIFO
        const S_IFIFO  = 1 << 12;

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
