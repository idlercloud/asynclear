//! 自旋锁，封装了一下 `spin::mutex::spin`
//!
//! 裁剪了一些不太需要的方法，添加 debug 模式下的死锁检测
//!
//! 未来有可能添加一些额外的操作（比如关中断等）

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
    /// Locks the [`SpinMutex`] and returns a guard that permits access to the
    /// inner data.
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
    /// Locks the [`SpinNoIrqMutex`] and returns a guard that permits access to
    /// the inner data.
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
        // Tests nested mutexes and access
        // to underlying data.
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

// 以下是直接从 spin 的源代码改过来的

// //! 自旋锁，修改自 spin crate 的 spin::mutex::spin 模块
// //!
// //! 裁剪了一些不太需要的方法。未来有可能添加一些额外的操作（比如关中断等）

// use core::{
//     cell::UnsafeCell,
//     fmt,
//     ops::{Deref, DerefMut},
//     sync::atomic::{AtomicBool, Ordering},
// };

// /// A [spin lock](https://en.m.wikipedia.org/wiki/Spinlock) providing mutually exclusive access to data.
// ///
// /// # Example
// ///
// /// ```
// /// use spin;
// ///
// /// let lock = spin::mutex::SpinMutex::<_>::new(0);
// ///
// /// // Modify the data
// /// *lock.lock() = 2;
// ///
// /// // Read the data
// /// let answer = *lock.lock();
// /// assert_eq!(answer, 2);
// /// ```
// ///
// /// # Thread safety example
// ///
// /// ```
// /// use spin;
// /// use std::sync::{Arc, Barrier};
// ///
// /// let thread_count = 1000;
// /// let spin_mutex = Arc::new(spin::mutex::SpinMutex::<_>::new(0));
// ///
// /// // We use a barrier to ensure the readout happens after all writing
// /// let barrier = Arc::new(Barrier::new(thread_count + 1));
// ///
// /// # let mut ts = Vec::new();
// /// for _ in (0..thread_count) {
// ///     let my_barrier = barrier.clone();
// ///     let my_lock = spin_mutex.clone();
// /// # let t =
// ///     std::thread::spawn(move || {
// ///         let mut guard = my_lock.lock();
// ///         *guard += 1;
// ///
// ///         // Release the lock to prevent a deadlock
// ///         drop(guard);
// ///         my_barrier.wait();
// ///     });
// /// # ts.push(t);
// /// }
// ///
// /// barrier.wait();
// ///
// /// let answer = { *spin_mutex.lock() };
// /// assert_eq!(answer, thread_count);
// ///
// /// # for t in ts {
// /// #     t.join().unwrap();
// /// # }
// /// ```
// pub struct SpinMutex<T: ?Sized> {
//     locked: AtomicBool,
//     data: UnsafeCell<T>,
// }

// // Same unsafe impls as `std::sync::Mutex`
// unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}
// unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

// impl<T> SpinMutex<T> {
//     /// Creates a new [`SpinMutex`] wrapping the supplied data.
//     ///
//     /// # Example
//     ///
//     /// ```
//     /// use spin::mutex::SpinMutex;
//     ///
//     /// static MUTEX: SpinMutex<()> = SpinMutex::<_>::new(());
//     ///
//     /// fn demo() {
//     ///     let lock = MUTEX.lock();
//     ///     // do something with lock
//     ///     drop(lock);
//     /// }
//     /// ```
//     #[inline(always)]
//     pub const fn new(data: T) -> Self {
//         SpinMutex {
//             locked: AtomicBool::new(false),
//             data: UnsafeCell::new(data),
//         }
//     }

//     /// Consumes this [`SpinMutex`] and unwraps the underlying data.
//     ///
//     /// # Example
//     ///
//     /// ```
//     /// let lock = spin::mutex::SpinMutex::<_>::new(42);
//     /// assert_eq!(42, lock.into_inner());
//     /// ```
//     #[inline(always)]
//     pub fn into_inner(self) -> T {
//         // We know statically that there are no outstanding references to
//         // `self` so there's no need to lock.
//         let SpinMutex { data, .. } = self;
//         data.into_inner()
//     }
// }

// impl<T: ?Sized> SpinMutex<T> {
//     /// Locks the [`SpinMutex`] and returns a guard that permits access to
// the inner data.     ///
//     /// The returned value may be dereferenced for data access
//     /// and the lock will be dropped when the guard falls out of scope.
//     ///
//     /// ```
//     /// let lock = spin::mutex::SpinMutex::<_>::new(0);
//     /// {
//     ///     let mut data = lock.lock();
//     ///     // The lock is now locked and the data can be accessed
//     ///     *data += 1;
//     ///     // The lock is implicitly dropped at the end of the scope
//     /// }
//     /// ```
//     #[inline]
//     pub fn lock(&self) -> SpinMutexGuard<'_, T> {
//         #[cfg(debug_assertions)]
//         let begin = riscv_time::get_time_ms();
//         // Can fail to lock even if the spinlock is not locked. May be more
// efficient than `try_lock`         // when called in a loop.
//         while self
//             .locked
//             .compare_exchange_weak(false, true, Ordering::Acquire,
// Ordering::Relaxed)             .is_err()
//         {
//             // Wait until the lock looks unlocked before retrying
//             while self.is_locked() {
//                 #[cfg(debug_assertions)]
//                 if begin - riscv_time::get_time_ms() >= 2000 {
//                     panic!("deadlock detected");
//                 }
//                 core::hint::spin_loop();
//             }
//         }

//         SpinMutexGuard { lock: self }
//     }

//     /// Returns `true` if the lock is currently held.
//     ///
//     /// # Safety
//     ///
//     /// This function provides no synchronization guarantees and so its
// result should be considered 'out of date'     /// the instant it is called.
// Do not use it for synchronization purposes. However, it may be useful as a
// heuristic.     #[inline(always)]
//     pub fn is_locked(&self) -> bool {
//         self.locked.load(Ordering::Relaxed)
//     }

//     /// Try to lock this [`SpinMutex`], returning a lock guard if successful.
//     ///
//     /// # Example
//     ///
//     /// ```
//     /// let lock = spin::mutex::SpinMutex::<_>::new(42);
//     ///
//     /// let maybe_guard = lock.try_lock();
//     /// assert!(maybe_guard.is_some());
//     ///
//     /// // `maybe_guard` is still held, so the second call fails
//     /// let maybe_guard2 = lock.try_lock();
//     /// assert!(maybe_guard2.is_none());
//     /// ```
//     #[inline]
//     pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
//         // The reason for using a strong compare_exchange is explained here:
//         // https://github.com/Amanieu/parking_lot/pull/207#issuecomment-575869107
//         if self
//             .locked
//             .compare_exchange(false, true, Ordering::Acquire,
// Ordering::Relaxed)             .is_ok()
//         {
//             Some(SpinMutexGuard { lock: self })
//         } else {
//             None
//         }
//     }

//     /// Returns a mutable reference to the underlying data.
//     ///
//     /// Since this call borrows the [`SpinMutex`] mutably, and a mutable
// reference is guaranteed to be exclusive in     /// Rust, no actual locking
// needs to take place -- the mutable borrow statically guarantees no locks
// exist. As     /// such, this is a 'zero-cost' operation.
//     ///
//     /// # Example
//     ///
//     /// ```
//     /// let mut lock = spin::mutex::SpinMutex::<_>::new(0);
//     /// *lock.get_mut() = 10;
//     /// assert_eq!(*lock.lock(), 10);
//     /// ```
//     #[inline(always)]
//     pub fn get_mut(&mut self) -> &mut T {
//         // We know statically that there are no other references to `self`,
// so         // there's no need to lock the inner mutex.
//         unsafe { &mut *self.data.get() }
//     }
// }

// impl<T: fmt::Debug> fmt::Debug for SpinMutex<T> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self.try_lock() {
//             Some(guard) => write!(f, "Mutex {{ data: ")
//                 .and_then(|()| (&*guard).fmt(f))
//                 .and_then(|()| write!(f, "}}")),
//             None => write!(f, "Mutex {{ <locked> }}"),
//         }
//     }
// }

// /// A guard that provides mutable data access.
// ///
// /// When the guard falls out of scope it will release the lock.
// pub struct SpinMutexGuard<'a, T: ?Sized> {
//     lock: &'a SpinMutex<T>,
// }

// unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}
// unsafe impl<T: ?Sized + Send> Send for SpinMutexGuard<'_, T> {}

// impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SpinMutexGuard<'a, T> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         fmt::Debug::fmt(&**self, f)
//     }
// }

// impl<'a, T: ?Sized + fmt::Display> fmt::Display for SpinMutexGuard<'a, T> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         fmt::Display::fmt(&**self, f)
//     }
// }

// impl<'a, T: ?Sized> Deref for SpinMutexGuard<'a, T> {
//     type Target = T;
//     fn deref(&self) -> &T {
//         // We know statically that only we are referencing data
//         unsafe { &*self.lock.data.get() }
//     }
// }

// impl<'a, T: ?Sized> DerefMut for SpinMutexGuard<'a, T> {
//     fn deref_mut(&mut self) -> &mut T {
//         // We know statically that only we are referencing data
//         unsafe { &mut *self.lock.data.get() }
//     }
// }

// impl<'a, T: ?Sized> Drop for SpinMutexGuard<'a, T> {
//     /// The dropping of the MutexGuard will release the lock it was created
// from.     fn drop(&mut self) {
//         self.lock.locked.store(false, Ordering::Release);
//     }
// }
