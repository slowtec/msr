use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

use thiserror::Error;

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
enum AtomicProgressHintSwitch {
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
    pub const fn default() -> Self {
        Self::new(ProgressHint::default())
    }

    const fn new(progress_hint: ProgressHint) -> Self {
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

    /// Load the current value
    pub fn load(&self) -> ProgressHint {
        Self::from_atomic_value(self.0.load(Ordering::Relaxed))
    }

    fn switch_from_expected_to_desired(
        &self,
        expected: AtomicValue,
        desired: AtomicValue,
    ) -> AtomicProgressHintSwitch {
        match self
            .0
            .compare_exchange(expected, desired, Ordering::AcqRel, Ordering::Relaxed)
        {
            Ok(_previous) => {
                debug_assert_eq!(expected, _previous);
                AtomicProgressHintSwitch::Accepted
            }
            Err(current) => {
                if current == desired {
                    AtomicProgressHintSwitch::Ignored
                } else {
                    AtomicProgressHintSwitch::Rejected
                }
            }
        }
    }

    fn switch_to_desired(&self, desired: AtomicValue) -> AtomicProgressHintSwitch {
        if self.0.swap(desired, Ordering::Release) == desired {
            AtomicProgressHintSwitch::Ignored
        } else {
            AtomicProgressHintSwitch::Accepted
        }
    }

    /// Switch from [`ProgressHint::Running`] to [`ProgressHint::Suspending`]
    pub fn suspend(&self) -> AtomicProgressHintSwitch {
        self.switch_from_expected_to_desired(PROGRESS_HINT_RUNNING, PROGRESS_HINT_SUSPENDING)
    }

    /// Switch from [`ProgressHint::Suspending`] to [`ProgressHint::Running`]
    pub fn resume(&self) -> AtomicProgressHintSwitch {
        self.switch_from_expected_to_desired(PROGRESS_HINT_SUSPENDING, PROGRESS_HINT_RUNNING)
    }

    /// Reset to [`ProgressHint::default()`]
    pub fn reset(&self) -> AtomicProgressHintSwitch {
        self.switch_to_desired(Self::to_atomic_value(ProgressHint::default()))
    }

    /// Set to [`ProgressHint::Terminating`]
    pub fn terminate(&self) -> AtomicProgressHintSwitch {
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
enum WaitForProgressHintSignalOutcome {
    Signaled,
    TimedOut,
}

/// The observed effect of switching the progress hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressHintSwitchOutcome {
    /// Changed as desired and signaled
    Accepted,

    /// Unchanged (i.e. already as desired) and silently ignored (i.e. not signaled)
    Ignored,
}

#[derive(Debug, Error)]
pub enum ProgressHintSwitchError {
    #[error("rejected")]
    Rejected,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type ProgressHintSwitchResult =
    std::result::Result<ProgressHintSwitchOutcome, ProgressHintSwitchError>;

#[allow(clippy::mutex_atomic)]
impl ProgressHintHandshake {
    pub fn load(&self) -> ProgressHint {
        self.atomic.load()
    }

    fn raise_signal_latch(&self) -> anyhow::Result<()> {
        let mut signal_latch_guard = self
            .signal_latch_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("signal poisened"))?;
        *signal_latch_guard = true;
        self.signal_latch_condvar.notify_one();
        Ok(())
    }

    fn reset_signal_latch(&self) -> anyhow::Result<()> {
        let mut signal_latch_guard = self
            .signal_latch_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
        *signal_latch_guard = false;
        Ok(())
    }

    fn after_atomic_switched(
        &self,
        switched: AtomicProgressHintSwitch,
    ) -> ProgressHintSwitchResult {
        match switched {
            AtomicProgressHintSwitch::Accepted => {
                self.raise_signal_latch()?;
                Ok(ProgressHintSwitchOutcome::Accepted)
            }
            AtomicProgressHintSwitch::Ignored => Ok(ProgressHintSwitchOutcome::Ignored),
            AtomicProgressHintSwitch::Rejected => Err(ProgressHintSwitchError::Rejected),
        }
    }

    pub fn suspend(&self) -> ProgressHintSwitchResult {
        self.after_atomic_switched(self.atomic.suspend())
    }

    pub fn resume(&self) -> ProgressHintSwitchResult {
        self.after_atomic_switched(self.atomic.resume())
    }

    pub fn terminate(&self) -> ProgressHintSwitchResult {
        self.after_atomic_switched(self.atomic.terminate())
    }

    pub fn reset(&self) -> anyhow::Result<()> {
        self.atomic.reset();
        self.reset_signal_latch()
    }

    pub fn wait_for_signal_with_timeout(
        &self,
        timeout: Duration,
    ) -> anyhow::Result<WaitForProgressHintSignalOutcome> {
        if timeout.is_zero() {
            // Time out immediately
            return Ok(WaitForProgressHintSignalOutcome::TimedOut);
        }
        let mut signal_latch_guard = self
            .signal_latch_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
        if *signal_latch_guard {
            // Reset the latch and abort immediately
            *signal_latch_guard = false;
            return Ok(WaitForProgressHintSignalOutcome::Signaled);
        }
        let (signal_latch_guard, wait_result) = self
            .signal_latch_condvar
            .wait_timeout_while(signal_latch_guard, timeout, |signal| {
                if *signal {
                    // Clear signal and abort waiting
                    *signal = false;
                    return false;
                }
                // Continue waiting
                true
            })
            .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
        // The signal latch has either not been raised or has been reset
        // while waiting. It cannot be raised again before we drop the
        // lock guard!
        assert!(!*signal_latch_guard);
        drop(signal_latch_guard);
        let outcome = if wait_result.timed_out() {
            WaitForProgressHintSignalOutcome::TimedOut
        } else {
            WaitForProgressHintSignalOutcome::Signaled
        };
        Ok(outcome)
    }

    pub fn wait_for_signal_with_deadline(
        &self,
        deadline: Instant,
    ) -> anyhow::Result<WaitForProgressHintSignalOutcome> {
        let now = Instant::now();
        let timeout = deadline.duration_since(deadline.min(now));
        self.wait_for_signal_with_timeout(timeout)
    }
}

#[derive(Debug, Clone)]
pub struct ProgressHintSender {
    handshake: Arc<ProgressHintHandshake>,
}

impl ProgressHintSender {
    /// Ask the receiver to suspend itself while running
    pub fn suspend(&self) -> ProgressHintSwitchResult {
        self.handshake.suspend()
    }

    /// Ask the receiver to resume itself while suspended
    pub fn resume(&self) -> ProgressHintSwitchResult {
        self.handshake.resume()
    }

    /// Ask the receiver to terminate itself
    pub fn terminate(&self) -> ProgressHintSwitchResult {
        self.handshake.terminate()
    }
}

#[derive(Debug, Default)]
pub struct ProgressHintReceiver {
    handshake: Arc<ProgressHintHandshake>,
}

impl ProgressHintReceiver {
    pub fn new_sender(&self) -> ProgressHintSender {
        let handshake = Arc::clone(&self.handshake);
        ProgressHintSender { handshake }
    }

    /// Reset the handshake
    ///
    /// Only the single receiver is allowed to reset the handshake.
    pub fn reset(&self) -> anyhow::Result<()> {
        self.handshake.reset()
    }

    /// Load the latest value
    ///
    /// Leave any pending handshake signals untouched.
    ///
    /// This function does not block and thus could be
    /// invoked safely in a real-time context.
    pub fn load(&self) -> ProgressHint {
        self.handshake.load()
    }

    /// Receive the latest progress hint, waiting for a signal
    ///
    /// Blocks until a new handshake signal has been received
    /// or the timeout has expired and then reads the latest
    /// value.
    ///
    /// This function might block and thus should not be
    /// invoked in a real-time context!
    pub fn recv_timeout(&self, timeout: Duration) -> anyhow::Result<ProgressHint> {
        let _outcome = self.handshake.wait_for_signal_with_timeout(timeout)?;
        Ok(self.load())
    }

    /// Receive the latest progress hint, waiting for a signal
    ///
    /// Blocks until a new handshake signal has been received
    /// or the deadline has expired and then reads the latest
    /// value.
    ///
    /// This function might block and thus should not be
    /// invoked in a real-time context!
    pub fn recv_deadline(&self, deadline: Instant) -> anyhow::Result<ProgressHint> {
        let _outcome = self.handshake.wait_for_signal_with_deadline(deadline);
        Ok(self.load())
    }
}

#[cfg(test)]
mod tests;
