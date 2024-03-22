//! 这里放的都是一些和用户库共享的信息，一般是一些有必要暴露给用户的结构体、枚举值等

#![no_std]
#![feature(panic_info_message)]
#![feature(format_args_nl)]

pub mod error;
pub mod misc;
pub mod signal;
pub mod syscall;
