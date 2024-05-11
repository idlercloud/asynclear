use common::constant::{MICRO_PER_SEC, NANO_PER_SEC};
use defines::{
    error::KResult,
    misc::{TimeSpec, TimeVal, Tms},
};
use user_check::UserCheckMut;

/// 获取自 Epoch 以来所过的时间（不过目前实现中似乎是自开机或复位以来时间）
///
/// 参数：
/// - `ts` 要设置的时间值
/// - `tz` 时区结构，但目前已经过时，不考虑
pub fn sys_get_time_of_day(tv: *mut TimeVal, _tz: usize) -> KResult {
    // 根据 man 所言，时区参数 `tz` 已经过时了，通常应当是 `NULL`。
    assert_eq!(_tz, 0);
    let mut tv = UserCheckMut::new(tv).check_ptr_mut()?;
    let us = riscv_time::get_time_us();
    tv.sec = us / MICRO_PER_SEC;
    tv.usec = us % MICRO_PER_SEC;
    Ok(0)
}

/// 全局时钟，或者说挂钟
const CLOCK_REALTIME: usize = 0;

/// 同样是获取时间，不过 `TimeSpec` 精度为 ns。
///
/// 可以有不同的时钟，但目前只支持挂钟 (`CLOCK_REALTIME`)。
///
/// 参数：
/// - `clock_id` 时钟 id，目前仅为 `CLOCK_READTIME`
/// - `tp` 指向要设置的用户指针
pub fn sys_clock_gettime(_clock_id: usize, ts: *mut TimeSpec) -> KResult {
    // TODO: 目前只考虑挂钟时间
    assert_eq!(_clock_id, CLOCK_REALTIME);
    let mut ts = UserCheckMut::new(ts).check_ptr_mut()?;
    let us = riscv_time::get_time_ns();
    ts.sec = (us / NANO_PER_SEC) as i64;
    ts.nsec = (us % NANO_PER_SEC) as i64;
    Ok(0)
}

// FIXME: `sys_times` 暂时是非正确的实现
/// 获取进程和子进程运行时间，单位是**时钟 tick 数**
///
/// 参数：
/// - `tms` 是一个用户指针，结果被写入其中。
///
/// 错误：
/// - `EFAULT` `tms` 指向非法地址
pub fn sys_times(tms: *mut Tms) -> KResult {
    let ticks = riscv::register::time::read();
    let mut tms = UserCheckMut::new(tms).check_ptr_mut()?;
    tms.tms_utime = ticks / 4;
    tms.tms_stime = ticks / 4;
    tms.tms_cutime = ticks / 4;
    tms.tms_cstime = ticks / 4;
    Ok(0)
}
