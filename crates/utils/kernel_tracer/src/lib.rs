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
pub use level::{Level, LevelFilter, CLOG, FLOG, SLOG};
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
    fn write_log(&self, writer: &mut impl Write, record: &Record<'_>, span_level: LevelFilter) {
        // 开头部分，即日志级别，如 `[ INFO]`
        let color = match record.level() {
            Level::Error => AnsiColor::Red,         // Red
            Level::Warn => AnsiColor::BrightYellow, // BrightYellow
            Level::Info => AnsiColor::Blue,         // Blue
            Level::Debug => AnsiColor::Green,       // Green
            Level::Trace => AnsiColor::BrightBlack, // BrightBlack
        };
        write!(
            writer,
            "{}[{:>5}]{}",
            color.render_fg(),
            record.level(),
            Reset.render()
        )
        .unwrap();

        // Span 栈部分
        let mut has_span = false;
        {
            let slab = self.slab.lock();
            let stack = self.span_stack.lock();

            const SPAN_NAME_COLOR: Style = AnsiColor::White.on_default().bold();

            for id in stack.iter() {
                let id = id.as_slab_index();
                let span_data = slab.get(id).unwrap();
                if span_data.level() > span_level {
                    continue;
                }
                has_span = true;

                write!(
                    writer,
                    "-{}{}{}",
                    SPAN_NAME_COLOR.render(),
                    span_data.name(),
                    Reset.render()
                )
                .unwrap();
                if let Some(kvs) = span_data.kvs() {
                    write!(writer, "{{{kvs}}}").unwrap();
                }
            }
        }
        if has_span {
            write!(writer, ": ").unwrap();
        } else {
            write!(writer, " ").unwrap();
        }

        // 日志信息部分
        writeln!(writer, "{}", record.args()).unwrap();
    }
}

#[inline]
#[doc(hidden)]
pub fn log(level: Level, args: core::fmt::Arguments<'_>) {
    let record = Record::new(level, args);
    if level <= crate::CLOG {
        KERNLE_TRACER.write_log(&mut *sbi_console::STDOUT.lock(), &record, CLOG);
    }
    if level <= crate::FLOG {
        static LOG_FS: Lazy<Mutex<DiskDriver>> = Lazy::new(|| Mutex::new(DiskDriver::new()));
        KERNLE_TRACER.write_log(&mut *LOG_FS.lock(), &record, FLOG);
    }
}
