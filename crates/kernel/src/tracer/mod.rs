#[cfg(feature = "profiling")]
extern crate alloc;

#[cfg(feature = "profiling")]
mod profiling;

use core::{
    fmt::Write,
    num::NonZeroU32,
    sync::atomic::{AtomicBool, Ordering},
};

use anstyle::{AnsiColor, Reset};
use console_output::{eprint, eprintln, STDOUT};
use kernel_tracer::{Level, Record, SpanAttr, SpanId, Tracer};
use klocks::{Lazy, SpinNoIrqMutex};
use libkernel::hart;
#[cfg(feature = "profiling")]
pub use profiling::report_profiling;
use slab::Slab;
#[cfg(feature = "profiling")]
use {alloc::vec::Vec, profiling::ProfilingEvent};

static KERNEL_TRACER_IMPL: Lazy<KernelTracerImpl> = Lazy::new(|| KernelTracerImpl {
    slab: SpinNoIrqMutex::new(Slab::with_capacity(64)),
    file_log_error_reported: AtomicBool::new(false),
    #[cfg(feature = "profiling")]
    events: SpinNoIrqMutex::new(Vec::with_capacity(128)),
});

pub fn init() {
    kernel_tracer::KERNEL_TRACER.call_once(|| Lazy::force(&KERNEL_TRACER_IMPL) as &(dyn Tracer + Sync));
}

/// 其实这个很可能是 ub，但是为了调试还是先这么写着吧
pub unsafe fn print_span_stack() {
    let span_stack = unsafe { &*hart::local_hart().span_stack.as_ptr() };
    let slab = KERNEL_TRACER_IMPL.slab.lock();
    eprintln!("span stack:");
    for id in span_stack.iter().rev() {
        let id = span_id_to_slab_index(id);
        let span_attr = slab.get(id).unwrap();
        eprint!("    {}: {}", span_attr.level(), span_attr.name());
        if let Some(kvs) = span_attr.kvs() {
            eprint!("{{{kvs}}}");
        }
        eprintln!();
    }
}

struct KernelTracerImpl {
    pub slab: SpinNoIrqMutex<Slab<SpanAttr>>,
    file_log_error_reported: AtomicBool,
    #[cfg(feature = "profiling")]
    pub events: SpinNoIrqMutex<Vec<ProfilingEvent>>,
}

impl Tracer for KernelTracerImpl {
    fn log_to_console(&self, record: &Record<'_>) {
        self.write_log(&mut *STDOUT.lock(), record, LogStyle::Color).unwrap();
    }

    fn log_to_file(&self, record: &Record<'_>) {
        let Some(console) = crate::drivers::file_log_console() else {
            return;
        };

        if console
            .write_with(|writer| self.write_log(writer, record, LogStyle::Plain))
            .is_err()
            && !self.file_log_error_reported.swap(true, Ordering::AcqRel)
        {
            eprintln!("failed to write kernel log to virtio log file, disabling file log");
        }
    }

    fn new_span(&self, span_attr: SpanAttr) -> SpanId {
        #[cfg(feature = "profiling")]
        let name = span_attr.name();
        let id = self.slab.lock().insert(span_attr);
        let id = NonZeroU32::new(id as u32 + 1).unwrap();
        #[cfg(feature = "profiling")]
        self.events.lock().push(ProfilingEvent::NewSpan { id: id.get(), name });
        SpanId::from_non_zero_u32(id)
    }

    #[track_caller]
    fn enter(&self, span_id: &SpanId) {
        hart::local_hart().span_stack.borrow_mut().push(span_id.clone());
        #[cfg(feature = "profiling")]
        self.events.lock().push(ProfilingEvent::Enter {
            hart_id: hart::local_hart().hart_id() as u32,
            id: span_id.to_u32(),
            instant: riscv_time::get_time_ns() as u64,
        });
    }

    fn exit(&self, span_id: &SpanId) {
        let _span_id = hart::local_hart().span_stack.borrow_mut().pop();
        #[cfg(feature = "profiling")]
        self.events.lock().push(ProfilingEvent::Exit {
            id: span_id.to_u32(),
            instant: riscv_time::get_time_ns() as u64,
        });
        // 维持一个栈结构，因此退出的 id 应当与进入的 id 保持一致
        debug_assert_eq!(_span_id.as_ref(), Some(span_id));
    }

    fn drop_span(&self, span_id: SpanId) {
        self.slab.lock().remove(span_id_to_slab_index(&span_id));
    }
}

const fn span_id_to_slab_index(span_id: &SpanId) -> usize {
    (span_id.to_u32() - 1) as usize
}

impl KernelTracerImpl {
    fn write_log(&self, mut writer: impl Write, record: &Record<'_>, style: LogStyle) -> core::fmt::Result {
        // 开头部分，即日志级别，如 `[ INFO]`
        match style {
            LogStyle::Color => {
                let log_color = output_color(record.level());
                write!(
                    writer,
                    "{}[{:>5}]{}",
                    log_color.render_fg(),
                    record.level(),
                    Reset.render()
                )?;
            }
            LogStyle::Plain => write!(writer, "[{:>5}]", record.level())?,
        }
        // Span 栈部分
        let mut has_span = false;
        {
            let slab = self.slab.lock();
            let stack = hart::local_hart().span_stack.borrow();

            for id in stack.iter() {
                let id = span_id_to_slab_index(id);
                let span_attr = slab.get(id).unwrap();
                has_span = true;

                match style {
                    LogStyle::Color => {
                        let span_style = output_color(span_attr.level()).on_default().bold();
                        write!(writer, "-{}{}{}", span_style.render(), span_attr.name(), Reset.render())?;
                    }
                    LogStyle::Plain => write!(writer, "-{}", span_attr.name())?,
                }
                if let Some(kvs) = span_attr.kvs() {
                    write!(writer, "{{{kvs}}}")?;
                }
            }
        }
        if has_span {
            write!(writer, ": ")?;
        } else {
            write!(writer, " ")?;
        }

        // 日志信息部分
        writeln!(writer, "{}", record.args())
    }
}

#[derive(Clone, Copy)]
enum LogStyle {
    Color,
    Plain,
}

fn output_color(level: Level) -> AnsiColor {
    match level {
        Level::Error => AnsiColor::Red,
        Level::Warn => AnsiColor::BrightYellow,
        Level::Info => AnsiColor::Blue,
        Level::Debug => AnsiColor::Green,
        Level::Trace => AnsiColor::BrightBlack,
    }
}
