pub mod instrument;

pub mod loggable;

use core::{
    fmt::{Debug, Write},
    num::NonZeroU32,
};

use ecow::EcoString;

use self::loggable::Loggable;
use crate::{Level, KERNEL_TRACER};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(NonZeroU32);

impl SpanId {
    #[inline]
    pub const fn from_non_zero_u32(id: NonZeroU32) -> Self {
        Self(id)
    }

    #[inline]
    pub const fn to_u32(&self) -> u32 {
        self.0.get()
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
        if let Some(tracer) = KERNEL_TRACER.get() {
            let kvs = kvs.map(|kvs| {
                let mut kvs_str = EcoString::new();
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

            let id = tracer.new_span(SpanAttr { level, name, kvs });
            Span { id: Some(id) }
        } else {
            Self::disabled()
        }
    }

    pub fn disabled() -> Self {
        Self { id: None }
    }

    #[track_caller]
    pub(crate) fn enter(&self) -> RefEnterGuard<'_> {
        if let Some(tracer) = KERNEL_TRACER.get() {
            if let Some(id) = &self.id {
                tracer.enter(id);
            }
        }
        RefEnterGuard { span: self }
    }

    #[track_caller]
    pub fn entered(self) -> OwnedEnterGuard {
        if let Some(tracer) = KERNEL_TRACER.get() {
            if let Some(id) = &self.id {
                tracer.enter(id);
            }
        }
        OwnedEnterGuard { span: self }
    }
}

impl Drop for Span {
    #[inline]
    fn drop(&mut self) {
        if let Some(tracer) = KERNEL_TRACER.get() {
            if let Some(id) = self.id.take() {
                tracer.drop_span(id);
            }
        }
    }
}

#[must_use = "once a span has been entered, it should be exited"]
pub struct RefEnterGuard<'a> {
    span: &'a Span,
}

// 不允许 Guard 越过 .await
impl !Send for RefEnterGuard<'_> {}

impl Drop for RefEnterGuard<'_> {
    fn drop(&mut self) {
        if let Some(tracer) = KERNEL_TRACER.get() {
            if let Some(id) = &self.span.id {
                tracer.exit(id);
            }
        }
    }
}

#[must_use = "once a span has been entered, it should be exited"]
pub struct OwnedEnterGuard {
    span: Span,
}

// 不允许 Guard 越过 .await
impl !Send for OwnedEnterGuard {}

impl Drop for OwnedEnterGuard {
    fn drop(&mut self) {
        if let Some(tracer) = KERNEL_TRACER.get() {
            if let Some(id) = &self.span.id {
                tracer.exit(id);
            }
        }
    }
}

pub struct SpanAttr {
    name: &'static str,
    level: Level,
    kvs: Option<EcoString>,
}

impl SpanAttr {
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
