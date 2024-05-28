//! 自旋锁，封装了一下 `spin::mutex::spin`
//!
//! 裁剪了一些不太需要的方法，添加 debug 模式下的死锁检测

use core::ops::{Deref, DerefMut};

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
        #[cfg(debug_assertions)]
        let begin = riscv_time::get_time_ms();
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            while self.is_locked() {
                core::hint::spin_loop();
                #[cfg(debug_assertions)]
                if riscv_time::get_time_ms() - begin >= 2000 {
                    panic!("deadlock detected");
                }
            }
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.base.get_mut()
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
    spin_guard: spin::mutex::SpinMutexGuard<'a, T>,
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
        #[cfg(debug_assertions)]
        let begin = riscv_time::get_time_ms();
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            while self.is_locked() {
                core::hint::spin_loop();
                #[cfg(debug_assertions)]
                if riscv_time::get_time_ms() - begin >= 2000 {
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
        let _no_irq_guard = riscv_guard::NoIrqGuard::new();
        self.base.try_lock().map(|spin_guard| SpinNoIrqMutexGuard {
            spin_guard,
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
