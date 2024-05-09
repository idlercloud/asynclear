//! 放一些比较杂又非常简单的东西，以至于不值得分出单独的文件

use core::time::Duration;

/// `sys_uname` 中指定的结构体类型。目前遵循 musl 的设置，每个字段硬编码为 65
/// 字节长
#[repr(C)]
pub struct UtsName {
    /// 系统名称
    pub sysname: [u8; 65],
    /// 网络上的主机名称
    pub nodename: [u8; 65],
    /// 发行编号
    pub release: [u8; 65],
    /// 版本
    pub version: [u8; 65],
    /// 硬件类型
    pub machine: [u8; 65],
    /// 域名
    pub domainname: [u8; 65],
}

const fn str_to_bytes(info: &str) -> [u8; 65] {
    let mut data: [u8; 65] = [0; 65];
    let mut index = 0;
    while index < info.len() {
        data[index] = info.as_bytes()[index];
        index += 1;
    }
    data
}

impl UtsName {
    pub const fn new() -> Self {
        Self {
            sysname: str_to_bytes("asynclear"),
            nodename: str_to_bytes("asynclear - machine[0]"),
            release: str_to_bytes("null"),
            version: str_to_bytes("0.1"),
            machine: str_to_bytes("qemu"),
            domainname: str_to_bytes("null"),
        }
    }
}

impl Default for UtsName {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpec {
    pub sec: i64,
    pub nsec: i64,
}

impl From<Duration> for TimeSpec {
    fn from(value: Duration) -> Self {
        Self {
            sec: value.as_secs() as i64,
            nsec: value.subsec_nanos() as i64,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
