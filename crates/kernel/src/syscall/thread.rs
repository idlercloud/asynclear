use defines::error::Result;

use crate::hart::local_hart;

/// TODO: 写注释
pub fn sys_gettid() -> Result {
    Ok(unsafe { (*local_hart()).curr_thread().tid } as isize)
}
