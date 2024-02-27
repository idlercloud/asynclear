use crate::span::Span;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use pin_project::pin_project;

pub trait Instrument: Future + Sized {
    fn instrument(self, span: Span) -> impl Future<Output = Self::Output> {
        Instrumented { inner: self, span }
    }
}

impl<T: Future> Instrument for T {}

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
struct Instrumented<F: Future + Sized> {
    #[pin]
    inner: F,
    span: Span,
}

impl<T: Future> Future for Instrumented<T> {
    type Output = T::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let project = self.project();
        let _enter = project.span.enter();
        project.inner.poll(cx)
    }
}
