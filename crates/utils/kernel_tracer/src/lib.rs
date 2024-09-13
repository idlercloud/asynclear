#![no_std]
#![feature(format_args_nl)]
#![feature(negative_impls)]
#![feature(decl_macro)]

extern crate alloc;

#[macro_use]
mod macros;
mod level;
mod record;
mod span;

use klocks::Once;
pub use level::{Level, CLOG, FLOG, SLOG};
pub use macros::{log, log_enabled};
pub use record::Record;
pub use span::{instrument::Instrument, Span, SpanAttr, SpanId};

pub static KERNEL_TRACER: Once<&'static (dyn Tracer + Sync)> = Once::new();

pub trait Tracer {
    fn log_to_console(&self, record: &Record<'_>);
    fn log_to_file(&self, record: &Record<'_>);
    fn new_span(&self, span_attr: SpanAttr) -> SpanId;
    #[track_caller]
    fn enter(&self, span_id: &SpanId);
    fn exit(&self, span_id: &SpanId);
    fn drop_span(&self, span_id: SpanId);
}

#[inline]
pub fn log(level: Level, args: core::fmt::Arguments<'_>) {
    if let Some(logger) = KERNEL_TRACER.get() {
        let record = Record::new(level, args);
        if level <= crate::CLOG {
            logger.log_to_console(&record);
        }
        if level <= crate::FLOG {
            logger.log_to_file(&record);
        }
    }
}
