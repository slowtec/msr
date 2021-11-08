use std::time::{Duration, Instant};

use crate::sync::{const_mutex, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalLatchState {
    Reset,
    Raised,
}

impl SignalLatchState {
    pub fn reset(&mut self) {
        *self = Self::default()
    }

    pub fn raise(&mut self) -> bool {
        match *self {
            Self::Raised => false,
            _ => {
                *self = Self::Raised;
                true
            }
        }
    }

    pub fn reset_if_raised(&mut self) -> bool {
        match *self {
            Self::Raised => {
                self.reset();
                true
            }
            Self::Reset => false,
        }
    }
}

impl SignalLatchState {
    pub const fn default() -> Self {
        Self::Reset
    }
}

impl Default for SignalLatchState {
    fn default() -> Self {
        Self::default()
    }
}

/// Stateful signaling between threads
///
/// Remembers when a signal is raised until a receiver of
/// the signal appears.
///
/// Allows to implement at-least-once semantics when performing
/// a handshake between threads, i.e. no signals get lost. Supposed
/// to be used in conjunction with an atomic variable that holds
/// the last (= most recent) value.
///
/// TODO: Replace this custom utility class with an appropriate
/// COTS implementation if available. Implementing low-level thread
/// synchronization primitives is hard, error prone, and should
/// be avoided whenever possible!
#[derive(Debug)]
pub struct SignalLatch {
    // TODO: Use crossbeam::sync::Parker/Unparker instead? Is it possible to
    // implement the crossbeam approach using a single, shared atomic value?
    // Probably not worth the effort.
    signal_latch_mutex: Mutex<SignalLatchState>,
    signal_latch_condvar: Condvar,
}

impl SignalLatch {
    pub const fn default() -> Self {
        Self {
            signal_latch_mutex: const_mutex(SignalLatchState::default()),
            signal_latch_condvar: Condvar::new(),
        }
    }
}

impl Default for SignalLatch {
    fn default() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitForSignalEvent {
    Raised,
    TimedOut,
}

impl SignalLatch {
    /// Raise the signal and wake up a single waiting thread
    ///
    /// Only wakes up threads on an edge trigger, i.e. if the
    /// signal state changed.
    ///
    /// Returns `true` if the signal has been raised and `false`
    /// if it was already raised.
    pub fn raise_notify_one(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.raise() {
            drop(signal_latch_guard);
            self.signal_latch_condvar.notify_one();
        }
    }

    /// Raise the signal and wake up all waiting threads
    ///
    /// Only wakes up threads on an edge trigger, i.e. if the
    /// signal state changed.
    ///
    /// Returns `true` if the signal has been raised and `false`
    /// if it was already raised.
    pub fn raise_notify_all(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.raise() {
            drop(signal_latch_guard);
            self.signal_latch_condvar.notify_all();
        }
    }

    /// Reset the signal if raised
    pub fn reset(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        signal_latch_guard.reset();
    }

    /// Wait until the signal is raised (blocking)
    ///
    /// Parks the calling thread until the signal is raised.
    ///
    /// The signal latch is reset when returning from this function.
    pub fn wait_for_signal(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.reset_if_raised() {
            // Abort immediately after resetting the latch
            return;
        }
        self.signal_latch_condvar.wait(&mut signal_latch_guard);
        // Reset the signal latch
        signal_latch_guard.reset();
    }

    /// Wait until the signal is raised or the timeout expired
    ///
    /// Parks the calling thread until one of the events occur.
    ///
    /// The signal latch is reset when returning from this function.
    ///
    /// Returns the event that occurred.
    pub fn wait_for_signal_with_timeout(&self, timeout: Duration) -> WaitForSignalEvent {
        if timeout.is_zero() {
            // Time out immediately
            return WaitForSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.reset_if_raised() {
            // Abort immediately after resetting the latch
            return WaitForSignalEvent::Raised;
        }
        let wait_result = self
            .signal_latch_condvar
            .wait_for(&mut signal_latch_guard, timeout);
        // Reset the signal latch
        signal_latch_guard.reset();
        drop(signal_latch_guard);
        if wait_result.timed_out() {
            WaitForSignalEvent::TimedOut
        } else {
            WaitForSignalEvent::Raised
        }
    }

    /// Wait until the signal is raised or the deadline expired
    ///
    /// Parks the calling thread until one of the events occur.
    ///
    /// The signal latch is reset when returning from this function.
    ///
    /// Returns the event that occurred.
    pub fn wait_for_signal_until_deadline(&self, deadline: Instant) -> WaitForSignalEvent {
        let now = Instant::now();
        if deadline <= now {
            // Time out immediately
            return WaitForSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.reset_if_raised() {
            // Abort immediately after resetting the latch
            return WaitForSignalEvent::Raised;
        }
        let wait_result = self
            .signal_latch_condvar
            .wait_until(&mut signal_latch_guard, deadline);
        // Reset the signal latch
        signal_latch_guard.reset();
        drop(signal_latch_guard);
        if wait_result.timed_out() {
            WaitForSignalEvent::TimedOut
        } else {
            WaitForSignalEvent::Raised
        }
    }
}

#[cfg(test)]
mod tests;
