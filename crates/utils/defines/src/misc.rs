//! 放一些比较杂又非常简单的东西，以至于不值得分出单独的文件

/// `sys_uname` 中指定的结构体类型。目前遵循 musl 的设置，每个字段硬编码为 65 字节长
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
    pub sec: usize,
    pub nsec: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}
