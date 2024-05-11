//! 自旋锁，封装了一下 `spin::mutex::spin`
//!
//! 裁剪了一些不太需要的方法，添加 debug 模式下的死锁检测

use core::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

pub struct SpinMutex<T: ?Sized> {
    base: spin::mutex::SpinMutex<T>,
}

pub struct SpinMutexGuard<'a, T: ?Sized> {
    // 要控制一下析构顺序，先释放锁再开中断
    inner: spin::mutex::SpinMutexGuard<'a, T>,
}

// Same unsafe impls as `std::sync::Mutex`
unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

// 不允许 Guard 越过 .await
impl<T: ?Sized> !Send for SpinMutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}

impl<T> SpinMutex<T> {
    /// Creates a new [`SpinMutex`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            base: spin::mutex::SpinMutex::new(data),
        }
    }
}

impl<T: ?Sized> SpinMutex<T> {
    /// Locks the [`SpinMutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    #[inline]
    #[track_caller]
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        #[cfg(all(debug_assertions, not(test)))]
        let begin = riscv_time::get_time_ms();
        #[cfg(test)]
        let begin = std::time::Instant::now();
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            while self.is_locked() {
                core::hint::spin_loop();
                #[cfg(all(debug_assertions, not(test)))]
                if riscv_time::get_time_ms() - begin >= 2000 {
                    panic!("deadlock detected");
                }
                #[cfg(test)]
                if begin.elapsed().as_millis() >= 2000 {
                    panic!("deadlock detected");
                }
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
    fn is_locked(&self) -> bool {
        self.base.is_locked()
    }

    /// Try to lock this [`SpinMutex`], returning a lock guard if successful.
    #[inline(always)]
    fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
        self.base.try_lock().map(|inner| SpinMutexGuard { inner })
    }
}

impl<'a, T: ?Sized> Deref for SpinMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // We know statically that only we are referencing data
        &self.inner
    }
}

impl<'a, T: ?Sized> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

pub struct SpinNoIrqMutex<T: ?Sized> {
    base: spin::mutex::SpinMutex<T>,
}

pub struct SpinNoIrqMutexGuard<'a, T: ?Sized> {
    // 要控制一下析构顺序，先释放锁再开中断
    spin_guard: ManuallyDrop<spin::mutex::SpinMutexGuard<'a, T>>,
    // 测试情况下一般不是跑在 riscv 上，因此无法使用 guard
    #[cfg(not(test))]
    _no_irq_guard: riscv_guard::NoIrqGuard,
}

unsafe impl<T: ?Sized + Send> Send for SpinNoIrqMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for SpinNoIrqMutex<T> {}

// 不允许 Guard 越过 .await
impl<T: ?Sized> !Send for SpinNoIrqMutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for SpinNoIrqMutexGuard<'_, T> {}

impl<T> SpinNoIrqMutex<T> {
    /// Creates a new [`SpinNoIrqMutex`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            base: spin::mutex::SpinMutex::new(data),
        }
    }
}

impl<T: ?Sized> SpinNoIrqMutex<T> {
    /// Locks the [`SpinNoIrqMutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    #[inline]
    #[track_caller]
    pub fn lock(&self) -> SpinNoIrqMutexGuard<'_, T> {
        #[cfg(all(debug_assertions, not(test)))]
        let begin = riscv_time::get_time_ms();
        #[cfg(test)]
        let begin = std::time::Instant::now();
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            while self.is_locked() {
                core::hint::spin_loop();
                #[cfg(all(debug_assertions, not(test)))]
                if riscv_time::get_time_ms() - begin >= 2000 {
                    panic!("deadlock detected");
                }
                #[cfg(test)]
                if begin.elapsed().as_millis() >= 2000 {
                    panic!("deadlock detected");
                }
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
    fn is_locked(&self) -> bool {
        self.base.is_locked()
    }

    /// Try to lock this [`SpinMutex`], returning a lock guard if successful.
    #[inline(always)]
    fn try_lock(&self) -> Option<SpinNoIrqMutexGuard<'_, T>> {
        #[cfg(not(test))]
        let _no_irq_guard = riscv_guard::NoIrqGuard::new();
        self.base.try_lock().map(|spin_guard| SpinNoIrqMutexGuard {
            spin_guard: ManuallyDrop::new(spin_guard),
            #[cfg(not(test))]
            _no_irq_guard,
        })
    }
}

impl<'a, T: ?Sized> Deref for SpinNoIrqMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // We know statically that only we are referencing data
        &self.spin_guard
    }
}

impl<'a, T: ?Sized> DerefMut for SpinNoIrqMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.spin_guard
    }
}

impl<'a, T: ?Sized> Drop for SpinNoIrqMutexGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: 只会在这里 drop，而且之后再也不会被用到
        unsafe {
            ManuallyDrop::drop(&mut self.spin_guard);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        prelude::v1::*,
        sync::{mpsc::channel, Arc},
        thread,
    };

    type SpinMutex<T> = super::SpinMutex<T>;

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let m = SpinMutex::<_>::new(());
        drop(m.lock());
        drop(m.lock());
    }

    #[test]
    fn lots_and_lots() {
        static M: SpinMutex<()> = SpinMutex::<_>::new(());
        static mut CNT: u32 = 0;
        const J: u32 = 1000;
        const K: u32 = 3;

        fn inc() {
            for _ in 0..J {
                unsafe {
                    let _g = M.lock();
                    CNT += 1;
                }
            }
        }

        let (tx, rx) = channel();
        let mut ts = Vec::new();
        for _ in 0..K {
            let tx2 = tx.clone();
            ts.push(thread::spawn(move || {
                inc();
                tx2.send(()).unwrap();
            }));
            let tx2 = tx.clone();
            ts.push(thread::spawn(move || {
                inc();
                tx2.send(()).unwrap();
            }));
        }

        drop(tx);
        for _ in 0..2 * K {
            rx.recv().unwrap();
        }
        assert_eq!(unsafe { CNT }, J * K * 2);

        for t in ts {
            t.join().unwrap();
        }
    }

    #[test]
    fn try_lock() {
        let mutex = SpinMutex::<_>::new(42);

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
    fn test_mutex_arc_nested() {
        // Tests nested mutexes and access to underlying data.
        let arc = Arc::new(SpinMutex::<_>::new(1));
        let arc2 = Arc::new(SpinMutex::<_>::new(arc));
        let (tx, rx) = channel();
        let t = thread::spawn(move || {
            let lock = arc2.lock();
            let lock2 = lock.lock();
            assert_eq!(*lock2, 1);
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
        t.join().unwrap();
    }

    #[test]
    fn test_mutex_arc_access_in_unwind() {
        let arc = Arc::new(SpinMutex::<_>::new(1));
        let arc2 = arc.clone();
        let _ = thread::spawn(move || -> () {
            struct Unwinder {
                i: Arc<SpinMutex<i32>>,
            }
            impl Drop for Unwinder {
                fn drop(&mut self) {
                    *self.i.lock() += 1;
                }
            }
            let _u = Unwinder { i: arc2 };
            panic!();
        })
        .join();
        let lock = arc.lock();
        assert_eq!(*lock, 2);
    }

    #[test]
    fn test_mutex_unsized() {
        let mutex: &SpinMutex<[i32]> = &SpinMutex::<_>::new([1, 2, 3]);
        {
            let b = &mut *mutex.lock();
            b[0] = 4;
            b[2] = 5;
        }
        let comp: &[i32] = &[4, 2, 5];
        assert_eq!(&*mutex.lock(), comp);
    }
}
