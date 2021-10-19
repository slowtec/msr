use std::sync::atomic::{AtomicU8, Ordering};

pub mod processor;
pub mod worker_thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Suspended,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressHint {
    /// Processing should continue
    Running,

    /// Processing should be suspended
    Suspending,

    /// Processing should be terminated
    Terminating,
}

const PROGRESS_HINT_RUNNING: u8 = 0;
const PROGRESS_HINT_SUSPENDING: u8 = 1;
const PROGRESS_HINT_TERMINATING: u8 = 2;

/// !!!DO NOT USE!!!
///
/// This type is only public for migrating legacy code.
#[derive(Debug)]
pub struct AtomicProgressHint(AtomicU8);

impl AtomicProgressHint {
    pub const fn new() -> Self {
        Self(AtomicU8::new(PROGRESS_HINT_RUNNING))
    }

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

    pub fn reset(&self) {
        self.0.store(PROGRESS_HINT_RUNNING, Ordering::Release);
    }

    pub fn terminate(&self) {
        self.0.store(PROGRESS_HINT_TERMINATING, Ordering::Release);
    }

    pub fn load(&self) -> ProgressHint {
        match self.0.load(Ordering::Acquire) {
            PROGRESS_HINT_RUNNING => ProgressHint::Running,
            PROGRESS_HINT_SUSPENDING => ProgressHint::Suspending,
            PROGRESS_HINT_TERMINATING => ProgressHint::Terminating,
            progress_hint => unreachable!("unexpected progress hint value: {}", progress_hint),
        }
    }
}
