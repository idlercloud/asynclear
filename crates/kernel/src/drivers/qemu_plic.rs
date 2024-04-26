//! 原始实现来自于 github.com/YdrMaster/plic
//!
//! 做了一些简化

use core::cell::UnsafeCell;

use common::config::{PA_TO_VA, QEMU_PLIC_ADDR};

/// See §1.
const COUNT_SOURCE: usize = 1024;
/// See §1.
const COUNT_CONTEXT: usize = 15872;
const U32_BITS: usize = u32::BITS as _;

#[repr(transparent)]
struct Priorities([UnsafeCell<u32>; COUNT_SOURCE]);

#[repr(transparent)]
struct PendingBits([UnsafeCell<u32>; COUNT_SOURCE / U32_BITS]);

#[repr(transparent)]
struct Enables([UnsafeCell<u32>; COUNT_SOURCE * COUNT_CONTEXT / U32_BITS]);

#[repr(C, align(4096))]
struct ContextLocal {
    priority_threshold: UnsafeCell<u32>,
    claim_or_completion: UnsafeCell<u32>,
    _reserved: [u8; 4096 - 2 * core::mem::size_of::<u32>()],
}

/// The PLIC memory mapping.
///
/// See §3.
#[repr(C, align(4096))]
pub struct Plic {
    priorities: Priorities,
    pending_bits: PendingBits,
    _reserved0: [u8; 4096 - core::mem::size_of::<PendingBits>()],
    enables: Enables,
    _reserved1: [u8; 0xe000],
    context_local: [ContextLocal; COUNT_CONTEXT],
}

impl Plic {
    pub fn mmio() -> *mut Plic {
        const PLIC_VA: usize = PA_TO_VA + QEMU_PLIC_ADDR;
        const {
            assert!(PLIC_VA % 4096 == 0);
        }
        PLIC_VA as *mut Plic
    }

    /// Sets priority for interrupt `source` to `value`.
    ///
    /// Write `0` to priority `value` effectively disables this interrupt `source`, for the priority
    /// value 0 is reserved for "never interrupt" by the PLIC specification.
    ///
    /// The lowest active priority is priority `1`. The maximum priority depends on PLIC implementation
    /// and can be detected with [`Plic::probe_priority_bits`].
    ///
    /// See §4.
    #[inline]
    pub fn set_priority(&self, source_id: usize, value: u32) {
        let ptr = self.priorities.0[source_id].get();
        unsafe { ptr.write_volatile(value) }
    }

    // /// Gets priority for interrupt `source`.
    // ///
    // /// See §4.
    // #[inline]
    // pub fn get_priority(&self, source_id: usize) -> u32 {
    //     let ptr = self.priorities.0[source_id].get();
    //     unsafe { ptr.read_volatile() }
    // }

    // /// Probe maximum level of priority for interrupt `source`.
    // ///
    // /// See §4.
    // #[inline]
    // pub fn probe_priority_bits(&self, source_id: usize) -> u32 {
    //     let ptr = self.priorities.0[source_id].get();
    //     unsafe {
    //         ptr.write_volatile(!0);
    //         ptr.read_volatile()
    //     }
    // }

    // /// Check if interrupt `source` is pending.
    // ///
    // /// See §5.
    // #[inline]
    // pub fn is_pending(&self, source_id: usize) -> bool {
    //     let group = source_id / U32_BITS;
    //     let index = source_id % U32_BITS;

    //     let ptr = self.pending_bits.0[group].get();
    //     (unsafe { ptr.read_volatile() } & (1 << index)) != 0
    // }

    /// Enable interrupt `source` in `context`.
    ///
    /// See §6.
    #[inline]
    pub fn enable(&self, source_id: usize, context_id: usize) {
        let pos = context_id * COUNT_SOURCE + source_id;
        let group = pos / U32_BITS;
        let index = pos % U32_BITS;

        let ptr = self.enables.0[group].get();
        unsafe { ptr.write_volatile(ptr.read_volatile() | (1 << index)) }
    }

    // /// Disable interrupt `source` in `context`.
    // ///
    // /// See §6.
    // #[inline]
    // pub fn disable(&self, source_id: usize, context_id: usize) {
    //     let pos = context_id * COUNT_SOURCE + source_id;
    //     let group = pos / U32_BITS;
    //     let index = pos % U32_BITS;

    //     let ptr = self.enables.0[group].get();
    //     unsafe { ptr.write_volatile(ptr.read_volatile() & !(1 << index)) }
    // }

    // /// Check if interrupt `source` is enabled in `context`.
    // ///
    // /// See §6.
    // #[inline]
    // pub fn is_enabled(&self, source_id: usize, context_id: usize) -> bool {
    //     let pos = context_id * COUNT_SOURCE + source_id;
    //     let group = pos / U32_BITS;
    //     let index = pos % U32_BITS;

    //     let ptr = self.enables.0[group].get();
    //     (unsafe { ptr.read_volatile() } & (1 << index)) != 0
    // }

    // /// Get interrupt threshold in `context`.
    // ///
    // /// See §7.
    // #[inline]
    // pub fn get_threshold(&self, context_id: usize) -> u32 {
    //     let ptr = self.context_local[context_id].priority_threshold.get();
    //     unsafe { ptr.read_volatile() }
    // }

    /// Set interrupt threshold for `context` to `value`.
    ///
    /// See §7.
    #[inline]
    pub fn set_threshold(&self, context_id: usize, value: u32) {
        let ptr = self.context_local[context_id].priority_threshold.get();
        unsafe { ptr.write_volatile(value) }
    }

    // /// Probe maximum supported threshold value the `context` supports.
    // ///
    // /// See §7.
    // #[inline]
    // pub fn probe_threshold_bits(&self, context_id: usize) -> u32 {
    //     let ptr = self.context_local[context_id].priority_threshold.get();
    //     unsafe {
    //         ptr.write_volatile(!0);
    //         ptr.read_volatile()
    //     }
    // }

    /// Claim an interrupt in `context`, returning its source.
    ///
    /// It is always legal for a hart to perform a claim even if `EIP` is not set.
    /// A hart could set threshold to maximum to disable interrupt notification, but it does not mean
    /// interrupt source has stopped to send interrupt signals. In this case, hart would instead
    /// poll for active interrupt by periodically calling the `claim` function.
    ///
    /// See §8.
    #[inline]
    pub fn claim(&self, context_id: usize) -> usize {
        let ptr = self.context_local[context_id].claim_or_completion.get();
        unsafe { ptr.read_volatile() as usize }
    }

    /// Mark that interrupt identified by `source` is completed in `context`.
    ///
    /// See §9.
    #[inline]
    pub fn complete(&self, context_id: usize, source_id: usize) {
        let ptr = self.context_local[context_id].claim_or_completion.get();
        unsafe { ptr.write_volatile(source_id as u32) }
    }
}
