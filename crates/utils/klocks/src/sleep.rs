//! 基于 `event_listener` 和自旋锁的睡眠锁

use core::{
    fmt,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use event_listener::{listener, Event};
use spin::mutex::SpinMutexGuard;

pub struct SleepMutex<T: ?Sized> {
    lock_ops: Event,
    base: spin::mutex::SpinMutex<T>,
}

pub struct SleepMutexGuard<'a, T: ?Sized> {
    spin_guard: ManuallyDrop<SpinMutexGuard<'a, T>>,
    mutex: &'a SleepMutex<T>,
}

unsafe impl<T: ?Sized + Send> Send for SleepMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for SleepMutex<T> {}

// 不允许 Guard 越过 .await
impl<T: ?Sized> !Send for SleepMutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for SleepMutexGuard<'_, T> {}

impl<T> SleepMutex<T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        SleepMutex {
            lock_ops: Event::new(),
            base: spin::mutex::SpinMutex::new(data),
        }
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.base.into_inner()
    }
}

impl<T: ?Sized> SleepMutex<T> {
    #[inline]
    pub async fn lock(&self) -> SleepMutexGuard<'_, T> {
        if let Some(guard) = self.try_lock() {
            return guard;
        }
        self.acquire_slow().await
    }

    #[cold]
    async fn acquire_slow(&self) -> SleepMutexGuard<'_, T> {
        loop {
            listener!(self.lock_ops => listener);
            // 在这中间有可能锁被释放了
            // 因此建立起监听之后要重新试着拿一下锁
            if let Some(guard) = self.try_lock() {
                return guard;
            }
            listener.await;
            // 被唤醒之后试着拿锁（是否有可能失败？）
            if let Some(guard) = self.try_lock() {
                return guard;
            }
        }
    }

    /// Returns `true` if the lock is currently held.
    ///
    /// # Safety
    ///
    /// This function provides no synchronization guarantees and so its result
    /// should be considered 'out of date' the instant it is called. Do not
    /// use it for synchronization purposes. However, it may be useful as a
    /// heuristic.
    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.base.is_locked()
    }

    #[inline(always)]
    pub fn try_lock(&self) -> Option<SleepMutexGuard<'_, T>> {
        self.base.try_lock().map(|spin_guard| SleepMutexGuard {
            spin_guard: ManuallyDrop::new(spin_guard),
            mutex: self,
        })
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.base.get_mut()
    }
}

impl<T: fmt::Debug> fmt::Debug for SleepMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ data: ")
                .and_then(|()| (*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Mutex {{ <locked> }}"),
        }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SleepMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SleepMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for SleepMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.spin_guard
    }
}

impl<'a, T: ?Sized> DerefMut for SleepMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.spin_guard
    }
}

impl<'a, T: ?Sized> Drop for SleepMutexGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: 只会在这里 drop，而且之后再也不会被用到
        unsafe {
            ManuallyDrop::drop(&mut self.spin_guard);
        }
        self.mutex.lock_ops.notify(1);
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use smol::channel;

    use super::SleepMutex;

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let number = Arc::new(SleepMutex::new(1000));
        let (tx, rx) = smol::channel::bounded(1);

        let number2 = Arc::clone(&number);
        let t = smol::spawn(async move {
            let mut locked = number2.lock().await;
            *locked = 10000;
            tx.send(()).await.unwrap();
            drop(locked);
        });

        smol::block_on(async move {
            rx.recv().await.unwrap();
            let locked = number.lock().await;
            assert_eq!(*locked, 10000);
            t.await;
        })
    }

    #[test]
    fn lots_and_lots() {
        static M: SleepMutex<()> = SleepMutex::new(());
        static mut CNT: u32 = 0;
        const J: u32 = 10000;
        const K: u32 = 300;

        async fn inc() {
            for _ in 0..J {
                let _g = M.lock().await;
                unsafe {
                    CNT += 1;
                }
            }
        }

        let (tx, rx) = channel::unbounded();
        let mut ts = Vec::new();
        for _ in 0..K {
            let tx2 = tx.clone();
            ts.push(smol::spawn(async move {
                inc().await;
                tx2.send(()).await.unwrap();
            }));
            let tx2 = tx.clone();
            ts.push(smol::spawn(async move {
                inc().await;
                tx2.send(()).await.unwrap();
            }));
        }

        drop(tx);
        smol::block_on(async move {
            for _ in 0..2 * K {
                rx.recv().await.unwrap();
            }
            assert_eq!(unsafe { CNT }, J * K * 2);

            for t in ts {
                t.await;
            }
        });
    }

    #[test]
    fn try_lock() {
        let mutex = SleepMutex::<_>::new(42);

        // First lock succeeds
        let a = mutex.try_lock();
        assert_eq!(a.as_ref().map(|r| **r), Some(42));

        // Additional lock fails
        let b = mutex.try_lock();
        assert!(b.is_none());

        // After dropping lock, it succeeds again
        ::core::mem::drop(a);
        let c = mutex.try_lock();
        assert_eq!(c.as_ref().map(|r| **r), Some(42));
    }

    #[test]
    fn test_into_inner() {
        let m = SleepMutex::<_>::new(NonCopy(10));
        assert_eq!(m.into_inner(), NonCopy(10));
    }

    #[test]
    fn test_into_inner_drop() {
        struct Foo(Arc<AtomicUsize>);
        impl Drop for Foo {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let num_drops = Arc::new(AtomicUsize::new(0));
        let m = SleepMutex::<_>::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        {
            let _inner = m.into_inner();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_mutex_arc_nested() {
        // Tests nested mutexes and access
        // to underlying data.
        let arc = Arc::new(SleepMutex::<_>::new(1));
        let arc2 = Arc::new(SleepMutex::<_>::new(arc));
        let (tx, rx) = channel::unbounded();
        let t = smol::spawn(async move {
            let lock = arc2.lock().await;
            let lock2 = lock.lock().await;
            assert_eq!(*lock2, 1);
            tx.send(()).await.unwrap();
        });
        smol::block_on(async move {
            rx.recv().await.unwrap();
            t.await
        });
    }

    #[test]
    fn test_mutex_unsized() {
        let mutex: &SleepMutex<[i32]> = &SleepMutex::new([1, 2, 3]);
        smol::block_on(async move {
            {
                let b = &mut *mutex.lock().await;
                b[0] = 4;
                b[2] = 5;
            }
            let comp: &[i32] = &[4, 2, 5];
            assert_eq!(&*mutex.lock().await, comp);
        });
    }
}
