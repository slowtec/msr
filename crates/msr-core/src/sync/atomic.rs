#[cfg(loom)]
pub(crate) use loom::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

#[cfg(not(loom))]
pub(crate) use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

/// An atomic flag
///
/// Uses acquire/release memory ordering semantics for
/// reliable handover.
#[derive(Debug, Default)]
pub struct OrderedAtomicFlag(AtomicBool);

impl OrderedAtomicFlag {
    pub fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }

    pub fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn check_and_reset(&self) -> bool {
        // If the CAS operation fails then the current value must have
        // been `false`. The ordering on failure is irrelevant since
        // the resulting value is discarded.
        self.0
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    pub fn peek(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn load(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
