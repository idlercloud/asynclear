//! 放一些比较杂又非常简单的东西，以至于不值得分出单独的文件

use core::time::Duration;

use bitflags::bitflags;

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

#[repr(C)]
pub struct Tms {
    /// 当前进程的用户态时间
    pub tms_utime: usize,
    /// 当前进程的内核态时间
    pub tms_stime: usize,
    /// 已被 wait 的子进程的用户态时间
    pub tms_cutime: usize,
    /// 已被 wait 的子进程的内核态时间
    pub tms_cstime: usize,
}

bitflags! {
    #[derive(Clone,Copy,Debug)]
    /// sys_wait4 的选项，描述等待方式
    pub struct WaitFlags: u32 {
        /// 如果没有符合条件的子进程，则立刻返回
        const WNOHANG = 1 << 0;
        /// 如果子线程被信号暂停，则也返回
        const WIMTRACED = 1 << 1;
        /// 如果子线程被信号恢复 (SIGCONT)，则也返回
        const WCONTINUED = 1 << 3;
    }

    /// sys_mmap 中使用，描述内存映射保护方式
    #[derive(Clone, Copy, Debug)]
    pub struct MmapProt: u32 {
        const PROT_NONE  = 0;
        const PROT_READ  = 1 << 0;
        const PROT_WRITE = 1 << 1;
        const PROT_EXEC  = 1 << 2;
    }

    /// `MAP_SHARED` 和 `MAP_PRIVATE` 二者有且仅有其一。
    #[derive(Clone, Copy, Debug)]
    pub struct MmapFlags: u32 {
        /// 该区域的映射对其他进程可见。若有底层文件，则更新被同步到底层文件上。
        const MAP_SHARED  = 1 << 0;
        /// 私有的 Cow 映射。其他进程不可见，也不会同步到底层文件。
        const MAP_PRIVATE = 1 << 1;

        /// 不只将 `addr` 作为 hint，而是确确实实要求映射在 `addr` 上。
        /// `addr` 必须良好地对齐，大部分情况下是 `PAGE_SIZE` 的整数倍即可。
        /// `addr` 和 `len` 指定一个映射范围，已有的和它重合的映射会被舍弃。
        /// 而如果指定的地址无法被映射，那么 `mmap()` 失败
        const MAP_FIXED     = 1 << 4;
        /// 匿名映射，没有底层文件。内容全部初始化为 0。`fd` 必须为 -1，`offset` 必须为 0。
        const MAP_ANONYMOUS = 1 << 5;
        /// 不为该映射保留 swap 空间。当实际物理内存不足时，可能造成内存溢出。
        const MAP_NORESERVE = 1 << 14;
    }

    /// 用于 sys_clone 的选项
    #[derive(Clone, Copy, Debug)]
    pub struct CloneFlags: u32 {
        /// 共享地址空间
        const CLONE_VM = 1 << 8;
        /// 共享文件系统新信息
        const CLONE_FS = 1 << 9;
        /// 共享文件描述符 (fd) 表
        const CLONE_FILES = 1 << 10;
        /// 共享信号处理函数
        const CLONE_SIGHAND = 1 << 11;
        // /// 创建指向子任务的 fd，用于 sys_pidfd_open
        // const CLONE_PIDFD = 1 << 12;
        // /// 用于 sys_ptrace
        // const CLONE_PTRACE = 1 << 13;
        // /// 指定父任务创建后立即阻塞，直到子任务退出才继续
        // const CLONE_VFORK = 1 << 14;
        // /// 指定子任务的 ppid 为当前任务的 ppid，相当于创建“兄弟”而不是“子女”
        // const CLONE_PARENT = 1 << 15;
        /// 作为一个“线程”被创建。具体来说，它同 CLONE_PARENT 一样设置 ppid，且不可被 wait
        const CLONE_THREAD = 1 << 16;
        // /// 子任务使用新的命名空间。目前还未用到
        // const CLONE_NEWNS = 1 << 17;
        /// 子任务共享同一组信号量。用于 sys_semop
        const CLONE_SYSVSEM = 1 << 18;
        /// 要求设置 tls
        const CLONE_SETTLS = 1 << 19;
        /// 要求在父任务的一个地址写入子任务的 tid
        const CLONE_PARENT_SETTID = 1 << 20;
        /// 要求将子任务的一个地址清零。这个地址会被记录下来，当子任务退出时会触发此处的 futex
        const CLONE_CHILD_CLEARTID = 1 << 21;
        /// 历史遗留的 flag，现在按 linux 要求应忽略
        const CLONE_DETACHED = 1 << 22;
        // /// 与 sys_ptrace 相关，目前未用到
        // const CLONE_UNTRACED = 1 << 23;
        // /// 要求在子任务的一个地址写入子任务的 tid
        // const CLONE_CHILD_SETTID = 1 << 24;
    }
}
