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
pub use record::Record;
pub use span::{instrument::Instrument, Span};

#[cfg(feature = "profiling")]
pub use span::ProfilingEvent;

use alloc::vec::Vec;
use klocks::{Lazy, Once, SpinNoIrqMutex};
use slab::Slab;
use span::{SpanData, SpanId};

// NOTE: 这里将 KernelTracer 都标记为 pub 其实不是好的
// 一般而言应该像 tracing 那样抽象出 trait 来分离

pub static KERNLE_TRACER: Lazy<KernelTracer> = Lazy::new(|| KernelTracer {
    logger: Once::new(),
    slab: SpinNoIrqMutex::new(Slab::with_capacity(64)),
    // TODO: 改造 span_stack 使其适应多核；并且可以用于栈展开
    span_stack: SpinNoIrqMutex::new(Vec::with_capacity(32)),
    #[cfg(feature = "profiling")]
    profiling_events: SpinNoIrqMutex::new(Vec::with_capacity(64)),
});

pub struct KernelTracer {
    pub logger: Once<&'static (dyn Log + Sync)>,
    pub slab: SpinNoIrqMutex<Slab<SpanData>>,
    pub span_stack: SpinNoIrqMutex<Vec<SpanId>>,
    #[cfg(feature = "profiling")]
    pub profiling_events: SpinNoIrqMutex<Vec<span::ProfilingEvent>>,
}

pub trait Log {
    fn log_to_console(&self, record: &Record<'_>);
    fn log_to_file(&self, record: &Record<'_>);
}

#[inline]
pub fn log(level: Level, args: core::fmt::Arguments<'_>) {
    let record = Record::new(level, args);
    if let Some(logger) = KERNLE_TRACER.logger.get() {
        if level <= crate::CLOG {
            logger.log_to_console(&record);
        }
        if level <= crate::FLOG {
            logger.log_to_file(&record);
        }
    }
}
