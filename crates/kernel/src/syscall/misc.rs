use defines::{
    error::KResult,
    misc::{SysInfo, UtsName},
};

use crate::{memory::UserCheck, time};

/// 返回系统信息，返回值为 0
pub fn sys_uname(utsname: UserCheck<UtsName>) -> KResult {
    let utsname = unsafe { utsname.check_ptr_mut()? };
    utsname.write(UtsName::default());
    Ok(0)
}

/// 读取与清除内核消息环形缓冲区
pub fn sys_syslog(log_type: u32, buf: UserCheck<[u8]>) -> KResult {
    unsafe {
        buf.check_slice_mut()?;
    }
    match log_type {
        2 | 3 | 4 => {
            // For type equal to 2, 3, or 4, a successful call to syslog() returns the number of bytes read.
            Ok(0)
        }
        9 => {
            // For type 9, syslog() returns the number of bytes currently available to be read on the kernel log buffer.
            Ok(0)
        }
        10 => {
            // For type 10, syslog() returns the total size of the kernel log buffer.  For other values of type, 0 is returned on success.
            Ok(0)
        }
        _ => {
            // For other values of type, 0 is returned on success.
            Ok(0)
        }
    }
}

/// 返回系统信息
pub fn sys_sysinfo(info: UserCheck<SysInfo>) -> KResult {
    let info = unsafe { info.check_ptr_mut()? };
    let sysinfo = SysInfo {
        uptime: time::curr_time().as_secs() as i64,
        ..Default::default()
    };
    info.write(sysinfo);
    Ok(0)
}
