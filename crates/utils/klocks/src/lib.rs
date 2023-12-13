#![cfg_attr(not(test), no_std)]

mod kspin;
mod sleep;

pub use kspin::{SpinMutex, SpinNoIrqMutex};
pub use sleep::SleepMutex;
pub use spin::Lazy;
