use core::{
    fmt::{Display, Write},
    marker::PhantomData,
    num::NonZeroU32,
};

use alloc::{string::String, vec::Vec};
use compact_str::CompactString;

use crate::{Level, KERNLE_TRACER};

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

pub trait Loggable {
    fn log(&self, writer: &mut CompactString);
}

// 要经过这个转一道。
// 不允许 impl<T: Display> Loggable for T 后再去给其他上游类型 impl Loggable
// 因为上游随时可能为该类型实现 Display，导致冲突
trait SpecDisplay: Display {}

macro_rules! mydisplay_impl {
    ($($t:tt)*) => ($(
        impl SpecDisplay for $t {}
    )*);
}

mydisplay_impl!(u8 u16 u32 u64 usize i8 i16 i32 i64 str char String CompactString);

impl<T: SpecDisplay + ?Sized> Loggable for T {
    fn log(&self, writer: &mut CompactString) {
        core::fmt::write(writer, format_args!("{self}")).unwrap();
    }
}

impl<T: SpecDisplay + ?Sized> SpecDisplay for &T {}

impl<T: SpecDisplay> Loggable for [T] {
    fn log(&self, writer: &mut CompactString) {
        writer.push_str("[");
        let mut rest = false;
        for t in self {
            if rest {
                writer.push_str(", ");
            }
            core::fmt::write(writer, format_args!("{t}")).unwrap();
            rest = true;
        }
        writer.push_str("]");
    }
}

impl<T: SpecDisplay> Loggable for Vec<T> {
    fn log(&self, writer: &mut CompactString) {
        self.as_slice().log(writer);
    }
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
            let mut i = 1;
            // this will not panic because
            // the macro implementation guarantee the array size > 0
            write!(kvs_str, "{}=", kvs[0].0).unwrap();
            kvs[0].1.log(&mut kvs_str);
            while i < kvs.len() {
                write!(kvs_str, " {}=", kvs[i].0).unwrap();
                kvs[i].1.log(&mut kvs_str);
                i += 1;
            }
            kvs_str
        });

        let span_data = SpanData { level, name, kvs };
        let id = KERNLE_TRACER.slab.lock().insert(span_data);
        Span {
            // this will always be `Some(_)`
            id: NonZeroU32::new(id as u32 + 1).map(SpanId),
        }
    }

    pub fn disabled() -> Self {
        Self { id: None }
    }

    pub(crate) fn enter(&self) -> RefEnterGuard<'_> {
        if let Some(id) = &self.id {
            KERNLE_TRACER.span_stack.lock().push(id.clone());
        }
        RefEnterGuard {
            span: self,
            _not_send: PhantomData,
        }
    }

    pub fn entered(self) -> OwnedEnterGuard {
        if let Some(id) = &self.id {
            KERNLE_TRACER.span_stack.lock().push(id.clone());
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
            let _span_id = KERNLE_TRACER.span_stack.lock().pop();
            // 维持一个栈结构，因此退出的 id 应当与进入的 id 保持一致
            debug_assert_eq!(_span_id.as_ref(), Some(id));
        }
    }
}

#[derive(Debug)]
pub struct SpanData {
    level: Level,
    name: &'static str,
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
