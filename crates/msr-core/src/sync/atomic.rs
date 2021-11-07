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

/// The observed effect of switching the progress hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchAtomicStateOk<T> {
    Accepted {
        previous_state: T,
    },

    /// Unchanged, i.e. already as desired
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchAtomicStateErr<T> {
    Rejected { current_state: T },
}

pub type SwitchAtomicStateResult<T> = Result<SwitchAtomicStateOk<T>, SwitchAtomicStateErr<T>>;

/// Atomic operations for state transitions
///
/// Needed for implementing state machines with atomic state transitions.
pub trait AtomicState {
    type State: Copy;

    /// Peek the current state
    ///
    /// Uses relaxed memory ordering semantics.
    fn peek(&self) -> Self::State;

    /// Load the current state
    ///
    /// Uses the same memory ordering semantics as when switching
    /// the state.
    fn load(&self) -> Self::State;

    /// Switch to the desired state unconditionally
    ///
    /// Replaces the current state with the desired state independent
    /// of the current state and returns the previous state.
    fn switch_to_desired(&self, desired_state: Self::State) -> SwitchAtomicStateOk<Self::State>;

    /// Switch to the desired state conditionally
    ///
    /// Replaces the current state with the desired state if it equals
    /// the given expected state and returns the previous state. Otherwise
    /// returns the unmodified current state.
    fn switch_from_expected_to_desired(
        &self,
        expected_state: Self::State,
        desired_state: Self::State,
    ) -> SwitchAtomicStateResult<Self::State>;
}
