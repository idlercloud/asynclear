#![cfg_attr(not(test), no_std)]
#![feature(negative_impls)]

mod kspin;
mod sleep;

pub use kspin::{SpinMutex, SpinNoIrqMutex};
pub use sleep::SleepMutex;
pub use spin::Lazy;
pub use spin::Once;
