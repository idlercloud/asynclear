#![cfg_attr(not(feature = "std"), no_std)]
#![feature(negative_impls)]

#[cfg(all(feature = "std", feature = "kernel"))]
compile_error!("Feature `std` 与 `kernel` 互斥，只能开启其中之一");

mod kspin;

pub use kspin::{SpinMutex, SpinMutexGuard, SpinNoIrqMutex, SpinNoIrqMutexGuard};
pub use spin::{
    rwlock::{RwLockReadGuard, RwLockWriteGuard},
    Lazy, Once, RwLock, Spin,
};
