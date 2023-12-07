pub mod instrument;

pub mod loggable;

use core::{
    fmt::{Debug, Write},
    marker::PhantomData,
    num::NonZeroU32,
};

use compact_str::CompactString;

use crate::{Level, KERNLE_TRACER};

use self::loggable::Loggable;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(NonZeroU32);

impl SpanId {
    pub fn as_slab_index(&self) -> usize {
        self.0.get() as usize - 1
    }
}

pub struct Span {
    id: Option<SpanId>,
}

impl Span {
    /// 创建一个新的 span。但只是将其注册，而没有实际实际启用。
    ///
    /// 调用 `entered()` 以进入该 span
    pub fn new<'a>(
        level: Level,
        name: &'static str,
        kvs: Option<&'a [(&'static str, &'a dyn Loggable)]>,
    ) -> Self {
        let kvs = kvs.map(|kvs| {
            let mut kvs_str = CompactString::new("");
            // this will not panic because
            // the macro implementation guarantee the array size > 0
            write!(kvs_str, "{}=", kvs[0].0).unwrap();
            kvs[0].1.log(&mut kvs_str);
            let mut i = 1;
            while i < kvs.len() {
                write!(kvs_str, " {}=", kvs[i].0).unwrap();
                kvs[i].1.log(&mut kvs_str);
                i += 1;
            }
            kvs_str
        });

        let span_data = SpanData { level, name, kvs };
        let id = KERNLE_TRACER.slab.lock().insert(span_data);
        let id = NonZeroU32::new(id as u32 + 1).unwrap();
        #[cfg(feature = "profiling")]
        #[cfg(feature = "profiling")]
        KERNLE_TRACER
            .profiling_events
            .lock()
            .push(ProfilingEvent::SetName { id: id.get(), name });
        Span {
            id: Some(SpanId(id)),
        }
    }

    pub fn disabled() -> Self {
        Self { id: None }
    }

    pub(crate) fn enter(&self) -> RefEnterGuard<'_> {
        if let Some(id) = &self.id {
            KERNLE_TRACER.span_stack.lock().push(id.clone());
            #[cfg(feature = "profiling")]
            KERNLE_TRACER
                .profiling_events
                .lock()
                .push(ProfilingEvent::Enter {
                    id: id.0.get(),
                    instant: riscv_time::get_time_ns() as u64,
                });
        }
        RefEnterGuard {
            span: self,
            _not_send: PhantomData,
        }
    }

    pub fn entered(self) -> OwnedEnterGuard {
        if let Some(id) = &self.id {
            KERNLE_TRACER.span_stack.lock().push(id.clone());
            #[cfg(feature = "profiling")]
            KERNLE_TRACER
                .profiling_events
                .lock()
                .push(ProfilingEvent::Enter {
                    id: id.0.get(),
                    instant: riscv_time::get_time_ns() as u64,
                });
        }
        OwnedEnterGuard {
            span: self,
            _not_send: PhantomData,
        }
    }
}

impl Drop for Span {
    #[inline]
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            KERNLE_TRACER.slab.lock().remove(id.as_slab_index());
        }
    }
}

#[must_use = "once a span has been entered, it should be exited"]
pub struct RefEnterGuard<'a> {
    span: &'a Span,
    _not_send: PhantomData<*const ()>,
}

impl Drop for RefEnterGuard<'_> {
    fn drop(&mut self) {
        if let Some(id) = &self.span.id {
            let _span_id = KERNLE_TRACER.span_stack.lock().pop();
            // 维持一个栈结构，因此退出的 id 应当与进入的 id 保持一致
            debug_assert_eq!(_span_id.as_ref(), Some(id));
        }
    }
}

#[must_use = "once a span has been entered, it should be exited"]
pub struct OwnedEnterGuard {
    span: Span,
    _not_send: PhantomData<*const ()>,
}

impl Drop for OwnedEnterGuard {
    fn drop(&mut self) {
        if let Some(id) = &self.span.id {
            #[cfg(feature = "profiling")]
            KERNLE_TRACER
                .profiling_events
                .lock()
                .push(ProfilingEvent::Exit {
                    instant: riscv_time::get_time_ns() as u64,
                });
            let _span_id = KERNLE_TRACER.span_stack.lock().pop();
            // 维持一个栈结构，因此退出的 id 应当与进入的 id 保持一致
            debug_assert_eq!(_span_id.as_ref(), Some(id));
        }
    }
}

pub struct SpanData {
    name: &'static str,
    level: Level,
    kvs: Option<CompactString>,
}

impl SpanData {
    pub fn level(&self) -> Level {
        self.level
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn kvs(&self) -> Option<&str> {
        self.kvs.as_deref()
    }
}

#[cfg(feature = "profiling")]
pub enum ProfilingEvent {
    SetName { id: u32, name: &'static str },
    Enter { id: u32, instant: u64 },
    Exit { instant: u64 },
}
