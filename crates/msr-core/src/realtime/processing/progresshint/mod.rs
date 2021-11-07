use std::time::{Duration, Instant};

use thiserror::Error;

use crate::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Condvar, Mutex, Weak,
};

/// Desired processing state
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

type AtomicValue = u8;

const PROGRESS_HINT_RUNNING: AtomicValue = 0;
const PROGRESS_HINT_SUSPENDING: AtomicValue = 1;
const PROGRESS_HINT_TERMINATING: AtomicValue = 2;

/// Atomic [`ProgressHint`]
#[derive(Debug)]
struct AtomicProgressHint(AtomicU8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchAtomicProgressHintOutcome {
    /// Changed
    Accepted,

    /// Already as desired and unchanged
    Ignored,

    /// Invalid transition and unchanged
    Rejected,
}

/// Atomic progress hint (thread-safe, lock-free)
///
/// Written by multiple non-realtime threads, read by
/// a single realtime thread.
///
/// Memory order for load operations: Relaxed
/// Memory order for store operations: Release
/// Memory order for load&store (CAS) operations: Acquire/Release
impl AtomicProgressHint {
    /// Default value
    ///
    /// Creates a new value in accordance to `ProgressHint::default()`.
    #[cfg(not(loom))]
    pub const fn default() -> Self {
        Self::new(ProgressHint::default())
    }

    // The loom atomic does not provide a const fn new()
    #[cfg(loom)]
    pub fn default() -> Self {
        Self::new(ProgressHint::default())
    }

    #[cfg(not(loom))]
    const fn new(progress_hint: ProgressHint) -> Self {
        Self(AtomicU8::new(Self::to_atomic_value(progress_hint)))
    }

    // The loom atomic does not provide a const fn new()
    #[cfg(loom)]
    fn new(progress_hint: ProgressHint) -> Self {
        Self(AtomicU8::new(Self::to_atomic_value(progress_hint)))
    }

    fn from_atomic_value(value: AtomicValue) -> ProgressHint {
        match value {
            PROGRESS_HINT_RUNNING => ProgressHint::Running,
            PROGRESS_HINT_SUSPENDING => ProgressHint::Suspending,
            PROGRESS_HINT_TERMINATING => ProgressHint::Terminating,
            progress_hint => unreachable!("unexpected progress hint value: {}", progress_hint),
        }
    }

    const fn to_atomic_value(progress_hint: ProgressHint) -> AtomicValue {
        match progress_hint {
            ProgressHint::Running => PROGRESS_HINT_RUNNING,
            ProgressHint::Suspending => PROGRESS_HINT_SUSPENDING,
            ProgressHint::Terminating => PROGRESS_HINT_TERMINATING,
        }
    }

    /// Read the current value with `relaxed` semantics (memory order)
    pub fn peek(&self) -> ProgressHint {
        Self::from_atomic_value(self.0.load(Ordering::Relaxed))
    }

    /// Read the current value with `acquire` semantics (memory order)
    pub fn load(&self) -> ProgressHint {
        Self::from_atomic_value(self.0.load(Ordering::Acquire))
    }

    fn switch_from_expected_to_desired(
        &self,
        expected: AtomicValue,
        desired: AtomicValue,
    ) -> SwitchAtomicProgressHintOutcome {
        match self
            .0
            .compare_exchange(expected, desired, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_previous) => {
                debug_assert_eq!(expected, _previous);
                SwitchAtomicProgressHintOutcome::Accepted
            }
            Err(current) => {
                if current == desired {
                    SwitchAtomicProgressHintOutcome::Ignored
                } else {
                    SwitchAtomicProgressHintOutcome::Rejected
                }
            }
        }
    }

    fn switch_to_desired(&self, desired: AtomicValue) -> SwitchAtomicProgressHintOutcome {
        if self.0.swap(desired, Ordering::Release) == desired {
            SwitchAtomicProgressHintOutcome::Ignored
        } else {
            SwitchAtomicProgressHintOutcome::Accepted
        }
    }

    /// Switch from [`ProgressHint::Running`] to [`ProgressHint::Suspending`]
    pub fn suspend(&self) -> SwitchAtomicProgressHintOutcome {
        self.switch_from_expected_to_desired(PROGRESS_HINT_RUNNING, PROGRESS_HINT_SUSPENDING)
    }

    /// Switch from [`ProgressHint::Suspending`] to [`ProgressHint::Running`]
    pub fn resume(&self) -> SwitchAtomicProgressHintOutcome {
        self.switch_from_expected_to_desired(PROGRESS_HINT_SUSPENDING, PROGRESS_HINT_RUNNING)
    }

    /// Reset to [`ProgressHint::default()`]
    pub fn reset(&self) -> SwitchAtomicProgressHintOutcome {
        self.switch_to_desired(Self::to_atomic_value(ProgressHint::default()))
    }

    /// Set to [`ProgressHint::Terminating`]
    pub fn terminate(&self) -> SwitchAtomicProgressHintOutcome {
        self.switch_to_desired(PROGRESS_HINT_TERMINATING)
    }
}

impl Default for AtomicProgressHint {
    fn default() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
#[allow(clippy::mutex_atomic)]
struct ProgressHintHandshake {
    atomic: AtomicProgressHint,
    signal_latch_mutex: Mutex<bool>,
    signal_latch_condvar: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitForProgressHintSignalEvent {
    Signaled,
    TimedOut,
}

/// The observed effect of switching the progress hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchProgressHintOk {
    /// Changed as desired and signaled
    Accepted,

    /// Unchanged (i.e. already as desired) and silently ignored (i.e. not signaled)
    Ignored,
}

#[derive(Debug, Error)]
pub enum SwitchProgressHintError {
    /// No receiver is attached
    ///
    /// The previously attached receiver has been dropped.
    ///
    /// Only occurs for the sender-side.
    #[error("detached")]
    Detached,

    /// The requested state transition is not permitted
    #[error("rejected")]
    Rejected,
}

pub type SwitchProgressHintResult = Result<SwitchProgressHintOk, SwitchProgressHintError>;

#[allow(clippy::mutex_atomic)]
impl ProgressHintHandshake {
    pub fn peek(&self) -> ProgressHint {
        self.atomic.peek()
    }

    pub fn load(&self) -> ProgressHint {
        self.atomic.load()
    }

    fn raise_signal_latch(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        *signal_latch_guard = true;
        self.signal_latch_condvar.notify_one();
    }

    fn reset_signal_latch(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        *signal_latch_guard = false;
    }

    fn after_atomic_switched(
        &self,
        switched: SwitchAtomicProgressHintOutcome,
    ) -> SwitchProgressHintResult {
        match switched {
            SwitchAtomicProgressHintOutcome::Accepted => {
                self.raise_signal_latch();
                Ok(SwitchProgressHintOk::Accepted)
            }
            SwitchAtomicProgressHintOutcome::Ignored => Ok(SwitchProgressHintOk::Ignored),
            SwitchAtomicProgressHintOutcome::Rejected => Err(SwitchProgressHintError::Rejected),
        }
    }

    pub fn suspend(&self) -> SwitchProgressHintResult {
        self.after_atomic_switched(self.atomic.suspend())
    }

    pub fn resume(&self) -> SwitchProgressHintResult {
        self.after_atomic_switched(self.atomic.resume())
    }

    pub fn terminate(&self) -> SwitchProgressHintResult {
        self.after_atomic_switched(self.atomic.terminate())
    }

    pub fn reset(&self) {
        self.atomic.reset();
        self.reset_signal_latch();
    }

    pub fn wait_for_signal_with_timeout(
        &self,
        timeout: Duration,
    ) -> WaitForProgressHintSignalEvent {
        if timeout.is_zero() {
            // Time out immediately
            return WaitForProgressHintSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if *signal_latch_guard {
            // Reset the latch and abort immediately
            *signal_latch_guard = false;
            return WaitForProgressHintSignalEvent::Signaled;
        }
        let wait_result = self
            .signal_latch_condvar
            .wait_for(&mut signal_latch_guard, timeout);
        // Reset the signal latch
        *signal_latch_guard = false;
        drop(signal_latch_guard);
        if wait_result.timed_out() {
            WaitForProgressHintSignalEvent::TimedOut
        } else {
            WaitForProgressHintSignalEvent::Signaled
        }
    }

    pub fn wait_for_signal_until_deadline(
        &self,
        deadline: Instant,
    ) -> WaitForProgressHintSignalEvent {
        let now = Instant::now();
        if deadline <= now {
            // Time out immediately
            return WaitForProgressHintSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if *signal_latch_guard {
            // Reset the latch and abort immediately
            *signal_latch_guard = false;
            return WaitForProgressHintSignalEvent::Signaled;
        }
        let wait_result = self
            .signal_latch_condvar
            .wait_until(&mut signal_latch_guard, deadline);
        // Reset the signal latch
        *signal_latch_guard = false;
        drop(signal_latch_guard);
        if wait_result.timed_out() {
            WaitForProgressHintSignalEvent::TimedOut
        } else {
            WaitForProgressHintSignalEvent::Signaled
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressHintSender {
    handshake: Weak<ProgressHintHandshake>,
}

impl ProgressHintSender {
    pub fn attach(rx: &ProgressHintReceiver) -> Self {
        let handshake = Arc::downgrade(&rx.handshake);
        ProgressHintSender { handshake }
    }

    pub fn is_attached(&self) -> bool {
        self.handshake.strong_count() > 0
    }

    fn upgrade_handshake(&self) -> Result<Arc<ProgressHintHandshake>, SwitchProgressHintError> {
        self.handshake
            .upgrade()
            .ok_or(SwitchProgressHintError::Detached)
    }

    /// Ask the receiver to suspend itself while running
    pub fn suspend(&self) -> SwitchProgressHintResult {
        self.upgrade_handshake()
            .and_then(|handshake| handshake.suspend())
    }

    /// Ask the receiver to resume itself while suspended
    pub fn resume(&self) -> SwitchProgressHintResult {
        self.upgrade_handshake()
            .and_then(|handshake| handshake.resume())
    }

    /// Ask the receiver to terminate itself
    pub fn terminate(&self) -> SwitchProgressHintResult {
        self.upgrade_handshake()
            .and_then(|handshake| handshake.terminate())
    }
}

#[derive(Debug, Default)]
pub struct ProgressHintReceiver {
    handshake: Arc<ProgressHintHandshake>,
}

impl ProgressHintReceiver {
    /// Read the latest progress hint (lock-free)
    ///
    /// Reads the current value using `relaxed` semantics (memory order)
    /// and leave any pending handshake signals untouched.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn peek(&self) -> ProgressHint {
        self.handshake.peek()
    }

    /// Read the latest progress hint (lock-free)
    ///
    /// Reads the current value using `acquire` semantics (memory order)
    /// and leave any pending handshake signals untouched.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn load(&self) -> ProgressHint {
        self.handshake.load()
    }

    /// Wait for a progress hint signal with a timeout (blocking)
    ///
    /// Blocks until a new handshake signal has been received
    /// or the timeout has expired.
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the signal
    /// could cause a priority inversion.
    pub fn wait_for_signal_with_timeout(
        &self,
        timeout: Duration,
    ) -> WaitForProgressHintSignalEvent {
        self.handshake.wait_for_signal_with_timeout(timeout)
    }

    /// Wait for a progress hint signal with a deadline (blocking)
    ///
    /// Blocks until a new handshake signal has been received
    /// or the deadline has expired.
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the signal
    /// could cause a priority inversion.
    pub fn wait_for_signal_until_deadline(
        &self,
        deadline: Instant,
    ) -> WaitForProgressHintSignalEvent {
        self.handshake.wait_for_signal_until_deadline(deadline)
    }

    /// Reset the handshake (blocking)
    ///
    /// Only the single receiver is allowed to reset the handshake.
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the signal
    /// could cause a priority inversion.
    pub fn reset(&self) {
        self.handshake.reset();
    }

    /// Detach all senders (lock-free)
    ///
    /// This will also reset the handshake back to default.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn detach(&mut self) {
        self.handshake = Default::default();
    }
}

#[cfg(test)]
mod tests;
