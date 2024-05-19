mod timer;

use core::time::Duration;

pub use self::timer::{check_timer, sleep};

/// 目前是返回自开机以来的 [`Duration`]
pub fn curr_time() -> Duration {
    let curr_ns = riscv_time::get_time_ns();
    Duration::from_nanos(curr_ns as u64)
}
