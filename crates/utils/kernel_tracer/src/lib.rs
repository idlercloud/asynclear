#![no_std]
#![feature(format_args_nl)]

extern crate alloc;

#[macro_use]
mod macros;
mod instrument;
mod level;
mod record;
mod span;

pub use instrument::Instrument;
pub use level::{Level, LevelFilter, CLOG, FLOG};
pub use record::Record;
pub use span::Span;

use core::fmt::Write;

use alloc::vec::Vec;
use anstyle::{AnsiColor, Reset, Style};
use drivers::DiskDriver;
use slab::Slab;
use span::{SpanData, SpanId};
use spin::{Lazy, Mutex};

pub static KERNLE_TRACER: Lazy<KernelTracer> = Lazy::new(|| KernelTracer {
    slab: Mutex::new(Slab::with_capacity(64)),
    // TODO: 改造 span_stack 使其适应多核；并且可以用于栈展开
    span_stack: Mutex::new(Vec::with_capacity(32)),
});

pub struct KernelTracer {
    slab: Mutex<Slab<SpanData>>,
    span_stack: Mutex<Vec<SpanId>>,
}

impl KernelTracer {
    #[inline]
    pub fn log_to_console(&self, record: &Record<'_>) {
        if record.level() <= CLOG {
            let color = match record.level() {
                Level::Error => AnsiColor::Red,         // Red
                Level::Warn => AnsiColor::BrightYellow, // BrightYellow
                Level::Info => AnsiColor::Blue,         // Blue
                Level::Debug => AnsiColor::Green,       // Green
                Level::Trace => AnsiColor::BrightBlack, // BrightBlack
            };
            let mut stdout = sbi_console::STDOUT.lock();

            write!(
                stdout,
                "{}[{:>5}]{} ",
                color.render_fg(),
                record.level(),
                Reset.render()
            )
            .unwrap();

            let slab = self.slab.lock();
            let stack = self.span_stack.lock();

            const SPAN_NAME_COLOR: Style = AnsiColor::White.on_default().bold();

            let mut has_span = false;

            for id in stack.iter() {
                let id = id.as_slab_index();
                let span_data = slab.get(id).unwrap();
                if span_data.level() > CLOG {
                    continue;
                }
                has_span = true;
                write!(
                    stdout,
                    "{}{}{}",
                    SPAN_NAME_COLOR.render(),
                    span_data.name(),
                    Reset.render()
                )
                .unwrap();
                if let Some(kvs) = span_data.kvs() {
                    write!(stdout, "{{{kvs}}}").unwrap();
                }
                write!(stdout, ":").unwrap();
            }

            if has_span {
                write!(stdout, " ").unwrap();
            }
            writeln!(stdout, "{}", record.args()).unwrap();
        }
    }

    #[inline]
    pub fn log_to_file(&self, record: &Record<'_>) {
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
#[doc(hidden)]
pub fn log(level: Level, args: core::fmt::Arguments<'_>) {
    let record = Record::new(level, args);
    if level <= crate::CLOG {
        crate::KERNLE_TRACER.log_to_console(&record);
    }
    if level <= crate::FLOG {
        crate::KERNLE_TRACER.log_to_file(&record);
    }
}
