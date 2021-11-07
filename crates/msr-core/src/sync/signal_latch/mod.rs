use std::time::{Duration, Instant};

use crate::sync::{const_mutex, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalLatchState {
    Empty,
    Signaled,
}

impl SignalLatchState {
    pub fn reset(&mut self) {
        *self = Self::default()
    }

    pub fn raise(&mut self) {
        *self = Self::Signaled
    }

    pub fn reset_if_raised(&mut self) -> bool {
        match *self {
            Self::Signaled => {
                self.reset();
                true
            }
            Self::Empty => false,
        }
    }
}

impl SignalLatchState {
    pub const fn default() -> Self {
        Self::Empty
    }
}

impl Default for SignalLatchState {
    fn default() -> Self {
        Self::default()
    }
}

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
    Signaled,
    TimedOut,
}

impl SignalLatch {
    pub fn raise_notify_one(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        signal_latch_guard.raise();
        self.signal_latch_condvar.notify_one();
    }

    pub fn raise_notify_all(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        signal_latch_guard.raise();
        self.signal_latch_condvar.notify_all();
    }

    pub fn reset(&self) {
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        signal_latch_guard.reset();
    }

    /// Wait for a signal with a timeout (blocking)
    ///
    /// Blocks until signal latch is raised or the timeout has expired.
    /// The signal latch is reset when returning from this function.
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the signal
    /// could cause a priority inversion.
    pub fn wait_with_timeout(&self, timeout: Duration) -> WaitForSignalEvent {
        if timeout.is_zero() {
            // Time out immediately
            return WaitForSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.reset_if_raised() {
            // Abort immediately after resetting the latch
            return WaitForSignalEvent::Signaled;
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
            WaitForSignalEvent::Signaled
        }
    }

    /// Wait for a signal with a deadline (blocking)
    ///
    /// Blocks until signal latch is raised or the deadline has expired.
    /// The signal latch is reset when returning from this function.
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the signal
    /// could cause a priority inversion.
    pub fn wait_until_deadline(&self, deadline: Instant) -> WaitForSignalEvent {
        let now = Instant::now();
        if deadline <= now {
            // Time out immediately
            return WaitForSignalEvent::TimedOut;
        }
        let mut signal_latch_guard = self.signal_latch_mutex.lock();
        if signal_latch_guard.reset_if_raised() {
            // Abort immediately after resetting the latch
            return WaitForSignalEvent::Signaled;
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
            WaitForSignalEvent::Signaled
        }
    }
}

#[cfg(test)]
mod tests;
