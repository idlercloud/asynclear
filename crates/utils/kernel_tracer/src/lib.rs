//! 综合参考 log、tracing、puffin 等库设计的内核用日志、跟踪、性能分析库
//!
//! 自己造轮子是因为一来有一些性能上的原因，希望可以优化；二来是这些库有些太过复杂，超出了内核的需求范围

#![no_std]
#![feature(format_args_nl)]

mod level;
#[macro_use]
mod macros;
mod record;

pub use level::{Level, LevelFilter, CLOG, FLOG};
pub use record::Record;

use core::fmt::Write;

use drivers::DiskDriver;
use sbi_console::println;
use spin::{Lazy, Mutex};

pub static KERNLE_TRACER: KernelTracer = KernelTracer {};

pub struct KernelTracer {}

impl KernelTracer {
    #[inline]
    pub fn log_to_console(&self, record: &Record) {
        if record.level() <= CLOG {
            let color = match record.level() {
                Level::Error => 31, // Red
                Level::Warn => 93,  // BrightYellow
                Level::Info => 34,  // Blue
                Level::Debug => 32, // Green
                Level::Trace => 90, // BrightBlack
            };
            println!(
                "\u{1B}[{}m[{:>5}] {}\u{1B}[0m",
                color,
                record.level(),
                record.args(),
            );
        }
    }

    #[inline]
    pub fn log_to_file(&self, record: &Record) {
        if record.level() <= FLOG {
            static LOG_FS: Lazy<Mutex<DiskDriver>> = Lazy::new(|| Mutex::new(DiskDriver::new()));
            writeln!(
                &mut LOG_FS.lock(),
                "[{:>5}] {}",
                record.level(),
                record.args()
            )
            .unwrap();
        }
    }
}

#[inline]
pub fn log_impl(level: Level, args: core::fmt::Arguments) {
    let record = Record::new(level, args);
    if level <= crate::CLOG {
        crate::KERNLE_TRACER.log_to_console(&record);
    }
    if level <= crate::FLOG {
        crate::KERNLE_TRACER.log_to_file(&record);
    }
}
