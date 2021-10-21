use std::sync::atomic::{AtomicU8, Ordering};

pub mod processor;

pub mod worker_thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressHint {
    /// Processing should continue
    Running,

    /// Processing should be suspended
    Suspending,

    /// Processing should be terminated
    Terminating,
}

impl ProgressHint {
    /// Default value
    ///
    /// The default should be used when no other information is available,
    /// i.e. processing should continue running uninterrupted.
    pub const fn default() -> Self {
        Self::Running
    }
}

impl Default for ProgressHint {
    fn default() -> Self {
        Self::default()
    }
}

const PROGRESS_HINT_RUNNING: u8 = 0;
const PROGRESS_HINT_SUSPENDING: u8 = 1;
const PROGRESS_HINT_TERMINATING: u8 = 2;

/// Atomic [`ProgressHint`]
#[derive(Debug)]
pub struct AtomicProgressHint(AtomicU8);

impl AtomicProgressHint {
    /// Default value
    ///
    /// Creates a new value in accordance to `ProgressHint::default()`.
    pub const fn default() -> Self {
        Self(AtomicU8::new(PROGRESS_HINT_RUNNING))
    }

    /// Switch from [`ProgressHint::Running`] to [`ProgressHint::Suspending`]
    ///
    /// Returns `true` if successful and `false` otherwise.
    pub fn suspend(&self) -> bool {
        self.0
            .compare_exchange(
                PROGRESS_HINT_RUNNING,
                PROGRESS_HINT_SUSPENDING,
                Ordering::Acquire,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Switch from [`ProgressHint::Suspending`] to [`ProgressHint::Running`]
    ///
    /// Returns `true` if successful and `false` otherwise.
    pub fn resume(&self) -> bool {
        self.0
            .compare_exchange(
                PROGRESS_HINT_SUSPENDING,
                PROGRESS_HINT_RUNNING,
                Ordering::Acquire,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Reset to [`ProgressHint::default()`]
    ///
    /// Returns `true` if successful and `false` otherwise.
    pub fn reset(&self) {
        self.0.store(PROGRESS_HINT_RUNNING, Ordering::Release);
    }

    /// Set to [`ProgressHint::Terminating`]
    pub fn terminate(&self) {
        self.0.store(PROGRESS_HINT_TERMINATING, Ordering::Release);
    }

    /// Load the current value
    ///
    /// The memory ordering *acquire* ensures that all subsequent operations
    /// are executed *after* the corresponding *store* operation.
    pub fn load(&self) -> ProgressHint {
        match self.0.load(Ordering::Acquire) {
            PROGRESS_HINT_RUNNING => ProgressHint::Running,
            PROGRESS_HINT_SUSPENDING => ProgressHint::Suspending,
            PROGRESS_HINT_TERMINATING => ProgressHint::Terminating,
            progress_hint => unreachable!("unexpected progress hint value: {}", progress_hint),
        }
    }
}

impl Default for AtomicProgressHint {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests;
