// TODO: 这个模块似乎可以拆出去？
// 出于简单性以及一些未来扩展的考虑，暂时先放在这里

use core::fmt::Write;

use anstyle::{AnsiColor, Reset, Style};
use drivers_hal::DiskDriver;
use kernel_tracer::{Level, Log, Record, KERNLE_TRACER};
use klocks::{Lazy, SpinNoIrqMutex};
use uart_console::STDOUT;

pub fn init() {
    KERNLE_TRACER.logger.call_once(|| &KernelLogImpl(()));
}

struct KernelLogImpl(());

impl Log for KernelLogImpl {
    fn log_to_console(&self, record: &Record<'_>) {
        write_log(&mut *STDOUT.lock(), record);
    }
    fn log_to_file(&self, record: &Record<'_>) {
        write_log(&mut *LOG_FS.lock(), record);
    }
}

fn write_log(writer: &mut impl Write, record: &Record<'_>) {
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
        let slab = KERNLE_TRACER.slab.lock();
        let stack = KERNLE_TRACER.span_stack.lock();

        const SPAN_NAME_COLOR: Style = AnsiColor::White.on_default().bold();

        for id in stack.iter() {
            let id = id.as_slab_index();
            let span_data = slab.get(id).unwrap();
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

static LOG_FS: Lazy<SpinNoIrqMutex<DiskDriver>> =
    Lazy::new(|| SpinNoIrqMutex::new(DiskDriver::new()));

#[cfg(feature = "profiling")]
pub fn report_profiling() {
    use kernel_tracer::ProfilingEvent;
    let mut fs = LOG_FS.lock();
    writeln!(fs, "<Profiling Report>").unwrap();
    for event in &*KERNLE_TRACER.profiling_events.lock() {
        match event {
            ProfilingEvent::SetName { id, name } => {
                writeln!(fs, "Setname: {id} => {name}").unwrap();
            }
            ProfilingEvent::Enter { id, instant } => {
                writeln!(fs, "Enter: {id} at {instant}ns").unwrap();
            }
            ProfilingEvent::Exit { instant } => writeln!(fs, "Exit: at {instant}ns").unwrap(),
        }
    }
}
