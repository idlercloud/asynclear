//! 这里放的都是一些和用户库共享的信息，一般是一些有必要暴露给用户的结构体、枚举值等

#![no_std]
#![feature(format_args_nl)]

extern crate alloc;

pub mod error;
pub mod fs;
pub mod ioctl;
pub mod misc;
pub mod resource;
pub mod signal;
pub mod syscall;
