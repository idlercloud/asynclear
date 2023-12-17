#![no_std]
#![feature(format_args_nl)]
#![feature(negative_impls)]

extern crate alloc;

#[macro_use]
mod macros;
mod level;
mod record;
mod span;

pub use level::{Level, CLOG, FLOG, SLOG};
pub use span::{instrument::Instrument, Span};

use core::fmt::Write;

use alloc::vec::Vec;
use anstyle::{AnsiColor, Reset, Style};
use klocks::{Lazy, SpinNoIrqMutex};
use level::LevelFilter;
use qemu_block::DiskDriver;
use record::Record;
use slab::Slab;
use span::{SpanData, SpanId};

static KERNLE_TRACER: Lazy<KernelTracer> = Lazy::new(|| KernelTracer {
    slab: SpinNoIrqMutex::new(Slab::with_capacity(64)),
    // TODO: 改造 span_stack 使其适应多核；并且可以用于栈展开
    span_stack: SpinNoIrqMutex::new(Vec::with_capacity(32)),
    #[cfg(feature = "profiling")]
    profiling_events: SpinNoIrqMutex::new(Vec::with_capacity(64)),
});

pub struct KernelTracer {
    slab: SpinNoIrqMutex<Slab<SpanData>>,
    span_stack: SpinNoIrqMutex<Vec<SpanId>>,
    #[cfg(feature = "profiling")]
    profiling_events: SpinNoIrqMutex<Vec<span::ProfilingEvent>>,
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
pub fn log(level: Level, args: core::fmt::Arguments<'_>) {
    let record = Record::new(level, args);
    if level <= crate::CLOG {
        KERNLE_TRACER.write_log(&mut *sbi_console::STDOUT.lock(), &record, CLOG);
    }
    if level <= crate::FLOG {
        KERNLE_TRACER.write_log(&mut *LOG_FS.lock(), &record, FLOG);
    }
}

static LOG_FS: Lazy<SpinNoIrqMutex<DiskDriver>> =
    Lazy::new(|| SpinNoIrqMutex::new(DiskDriver::new()));

#[cfg(feature = "profiling")]
pub fn report_profiling() {
    let mut fs = LOG_FS.lock();
    writeln!(fs, "<Profiling Report>").unwrap();
    for event in &*KERNLE_TRACER.profiling_events.lock() {
        match event {
            span::ProfilingEvent::SetName { id, name } => {
                writeln!(fs, "Setname: {id} => {name}").unwrap();
            }
            span::ProfilingEvent::Enter { id, instant } => {
                writeln!(fs, "Enter: {id} at {instant}ns").unwrap();
            }
            span::ProfilingEvent::Exit { instant } => writeln!(fs, "Exit: at {instant}ns").unwrap(),
        }
    }
}
