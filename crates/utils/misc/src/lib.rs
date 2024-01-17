#![no_std]

pub struct Deferred<T: FnOnce()> {
    f: Option<T>,
}

impl<T: FnOnce()> Deferred<T> {
    pub fn new(f: T) -> Deferred<T> {
        Self { f: Some(f) }
    }
}

impl<T: FnOnce()> Drop for Deferred<T> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}
