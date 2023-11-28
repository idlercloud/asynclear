#![no_std]
#![feature(format_args_nl)]

extern crate alloc;
#[macro_use]
extern crate sbi_console;

use core::fmt::Write;

use drivers::DiskDriver;
use log::{self, Level, LevelFilter, Log, Metadata, Record};
use spin::{Lazy, Mutex};

/// a simple logger
struct SimpleLogger {
    clog: LevelFilter,
    flog: LevelFilter,
}

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if self.clog >= record.level() {
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
        if self.flog >= record.level() {
            writeln!(
                &mut LOG_FS.lock(),
                "[{:>5}] {}",
                record.level(),
                record.args()
            )
            .unwrap();
        }
    }
    fn flush(&self) {}
}

static LOG_FS: Lazy<Mutex<DiskDriver>> = Lazy::new(|| Mutex::new(DiskDriver::new()));

/// initiate logger
pub fn init() {
    static LOGGER: Lazy<SimpleLogger> = Lazy::new(|| {
        let clog = match option_env!("KERNEL_CLOG") {
            Some("ERROR") => LevelFilter::Error,
            Some("WARN") => LevelFilter::Warn,
            Some("INFO") => LevelFilter::Info,
            Some("DEBUG") => LevelFilter::Debug,
            Some("TRACE") => LevelFilter::Trace,
            _ => LevelFilter::Off,
        };

        let flog = match option_env!("KERNEL_FLOG") {
            Some("ERROR") => LevelFilter::Error,
            Some("WARN") => LevelFilter::Warn,
            Some("INFO") => LevelFilter::Info,
            Some("DEBUG") => LevelFilter::Debug,
            Some("TRACE") => LevelFilter::Trace,
            _ => LevelFilter::Off,
        };
        SimpleLogger { clog, flog }
    });
    Lazy::force(&LOG_FS);

    log::set_logger(&*LOGGER).unwrap();

    log::set_max_level(LOGGER.clog.max(LOGGER.flog));
}
