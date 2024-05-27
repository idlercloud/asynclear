/// 对资源没有限制
pub const RLIM_INFINITY: usize = usize::MAX;

#[allow(unused)]
const RLIMIT_CPU: u32 = 0;
#[allow(unused)]
const RLIMIT_FSIZE: u32 = 1;
#[allow(unused)]
const RLIMIT_DATA: u32 = 2;
#[allow(unused)]
const RLIMIT_STACK: u32 = 3;
#[allow(unused)]
const RLIMIT_CORE: u32 = 4;
#[allow(unused)]
const RLIMIT_RSS: u32 = 5;
#[allow(unused)]
const RLIMIT_NPROC: u32 = 6;
pub const RLIMIT_NOFILE: u32 = 7;
#[allow(unused)]
const RLIMIT_MEMLOCK: u32 = 8;
#[allow(unused)]
const RLIMIT_AS: u32 = 9;
#[allow(unused)]
const RLIMIT_LOCKS: u32 = 10;
#[allow(unused)]
const RLIMIT_SIGPENDING: u32 = 11;
#[allow(unused)]
const RLIMIT_MSGQUEUE: u32 = 12;
#[allow(unused)]
const RLIMIT_NICE: u32 = 13;
#[allow(unused)]
const RLIMIT_RTPRIO: u32 = 14;
#[allow(unused)]
const RLIMIT_RTTIME: u32 = 15;

/// Resource Limit
#[derive(Debug, Clone, Copy)]
pub struct RLimit {
    /// 软上限，即当前的限制值
    pub rlim_curr: usize,
    /// 硬上限，即软上限的最大值。
    ///
    /// 非特权进程的软上限范围为 `0..=rlimi_max`，且只能（不可逆地）降低 `rlimix_max`
    pub rlim_max: usize,
}
