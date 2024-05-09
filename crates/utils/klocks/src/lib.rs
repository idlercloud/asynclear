#![cfg_attr(not(test), no_std)]
#![feature(negative_impls)]

mod kspin;
mod sleep;

pub use kspin::{SpinMutex, SpinMutexGuard, SpinNoIrqMutex, SpinNoIrqMutexGuard};
pub use sleep::SleepMutex;
pub use spin::{
    rwlock::{RwLockReadGuard, RwLockWriteGuard},
    Lazy, Once, RwLock, Spin,
};
