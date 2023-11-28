// use crate::task::{__block_curr_and_run_next, add_timer, check_ptr, curr_task};
// use utils::{
//     error::Result,
//     time::{get_time_ms, TimeSpec},
// };

// pub fn sys_sleep(req: *const TimeSpec) -> Result {
//     let req = unsafe { check_ptr(req) }?;
//     let expire_ms = get_time_ms() + req.sec * 1000 + req.nsec / 1_000_000;
//     let thread = curr_task().unwrap();
//     add_timer(expire_ms, thread);
//     __block_curr_and_run_next();
//     Ok(0)
// }
