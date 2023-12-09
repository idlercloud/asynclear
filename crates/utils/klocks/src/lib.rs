#![cfg_attr(not(test), no_std)]

mod kspin;
mod sleep;

pub use kspin::SpinMutex;
pub use sleep::SleepMutex;
pub use spin::{mutex::SpinMutexGuard, Lazy};
