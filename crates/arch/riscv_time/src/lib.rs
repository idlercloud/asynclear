#![no_std]

use defines::{
    config::{CLOCK_FREQ, TICKS_PER_SEC},
    constant::{MICRO_PER_SEC, MILLI_PER_SEC, NANO_PER_SEC},
};
use riscv::register::time;

/// 应当是返回时钟次数。应该是从开机或者复位算起。
#[inline]
pub fn get_time() -> usize {
    // 我记得 RISC-V 似乎有规定 mtime 寄存器无论 RV32 还是 RV64 都是 64 位精度的？
    // 但既然人家的库返回 usize，这里也就返回 usize 吧
    time::read()
}

#[inline]
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}

#[inline]
pub fn get_time_ms() -> usize {
    time::read() * MILLI_PER_SEC / CLOCK_FREQ
}

#[inline]
pub fn get_time_ns() -> usize {
    time::read() * NANO_PER_SEC / CLOCK_FREQ
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    sbi_rt::set_timer((get_time() + CLOCK_FREQ / TICKS_PER_SEC) as u64);
}
