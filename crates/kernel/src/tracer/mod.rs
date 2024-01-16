#[cfg(feature = "profiling")]
mod profiling;

#[cfg(feature = "profiling")]
pub use profiling::report_profiling;

use core::{fmt::Write, num::NonZeroU32};
#[cfg(feature = "profiling")]
use {alloc::vec::Vec, profiling::ProfilingEvent};

use anstyle::{AnsiColor, Reset, Style};
use drivers_hal::DiskDriver;
use kernel_tracer::{Level, Record, SpanAttr, SpanId, Tracer};
use klocks::{Lazy, SpinNoIrqMutex};
use slab::Slab;
use uart_console::STDOUT;

use crate::hart::{local_hart, local_hart_mut};

#[cfg(not(feature = "ktest"))]
static KERNEL_TRACER_IMPL: klocks::Once<KernelTracerImpl> = klocks::Once::new();

#[cfg(not(feature = "ktest"))]
pub fn init() {
    kernel_tracer::KERNLE_TRACER.call_once(|| {
        KERNEL_TRACER_IMPL.call_once(|| {
            // TODO: 改造 KernelLogImpl 使其适应多核；并且可以用于栈展开
            KernelTracerImpl {
                slab: SpinNoIrqMutex::new(Slab::with_capacity(64)),
                #[cfg(feature = "profiling")]
                events: SpinNoIrqMutex::new(Vec::with_capacity(128)),
            }
        })
    });
}

struct KernelTracerImpl {
    pub slab: SpinNoIrqMutex<Slab<SpanAttr>>,
    #[cfg(feature = "profiling")]
    pub events: SpinNoIrqMutex<Vec<ProfilingEvent>>,
}

impl Tracer for KernelTracerImpl {
    fn log_to_console(&self, record: &Record<'_>) {
        self.write_log(&mut *STDOUT.lock(), record);
    }

    fn log_to_file(&self, record: &Record<'_>) {
        self.write_log(&mut *LOG_FS.lock(), record);
    }

    fn new_span(&self, span_attr: SpanAttr) -> SpanId {
        #[cfg(feature = "profiling")]
        let name = span_attr.name();
        let id = self.slab.lock().insert(span_attr);
        let id = NonZeroU32::new(id as u32 + 1).unwrap();
        #[cfg(feature = "profiling")]
        self.events
            .lock()
            .push(ProfilingEvent::NewSpan { id: id.get(), name });
        SpanId::from_non_zero_u32(id)
    }

    fn enter(&self, span_id: &SpanId) {
        unsafe {
            (*local_hart_mut()).span_stack.push(span_id.clone());
        }
        #[cfg(feature = "profiling")]
        self.events.lock().push(ProfilingEvent::Enter {
            hart_id: unsafe { (*local_hart()).hart_id() as u32 },
            id: span_id.to_u32(),
            instant: riscv_time::get_time_ns() as u64,
        });
    }

    fn exit(&self, span_id: &SpanId) {
        let _span_id = unsafe { (*local_hart_mut()).span_stack.pop() };
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

fn span_id_to_slab_index(span_id: &SpanId) -> usize {
    (span_id.to_u32() - 1) as usize
}

impl KernelTracerImpl {
    fn write_log(&self, writer: &mut impl Write, record: &Record<'_>) {
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
            let stack = unsafe { &(*local_hart()).span_stack };

            const SPAN_NAME_COLOR: Style = AnsiColor::White.on_default().bold();

            for id in stack.iter() {
                let id = span_id_to_slab_index(id);
                let span_attr = slab.get(id).unwrap();
                has_span = true;

                write!(
                    writer,
                    "-{}{}{}",
                    SPAN_NAME_COLOR.render(),
                    span_attr.name(),
                    Reset.render()
                )
                .unwrap();
                if let Some(kvs) = span_attr.kvs() {
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

static LOG_FS: Lazy<SpinNoIrqMutex<DiskDriver>> =
    Lazy::new(|| SpinNoIrqMutex::new(DiskDriver::new()));
