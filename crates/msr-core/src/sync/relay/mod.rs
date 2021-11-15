use std::time::{Duration, Instant};

use crate::sync::{const_mutex, Condvar, Mutex};

/// Move single values between threads
///
/// A condition variable with a single slot that allows to pass
/// values from producer to consumer threads. Producers and consumers
/// may arrive at any point in time.
///
/// A typical scenario involves only a single producer and a single
/// consumer thread implementing a handover protocol for passing
/// the latest (= most recent) value between each other.
///
/// The value is buffered until the consumer is ready to take it.
/// Each value can be consumed at most once. Producers can replace
/// the current value if it has not been consumed yet.
#[derive(Debug)]
pub struct Relay<T> {
    mutex: Mutex<Option<T>>,
    condvar: Condvar,
}

impl<T> Relay<T> {
    pub const fn new() -> Self {
        Self {
            mutex: const_mutex(None),
            condvar: Condvar::new(),
        }
    }

    pub const fn with_value(value: T) -> Self {
        Self {
            mutex: const_mutex(Some(value)),
            condvar: Condvar::new(),
        }
    }
}

impl<T> Default for Relay<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Relay<T> {
    /// Replace the current value and notify a single waiting consumer
    ///
    /// Returns the previous value or `None`. If `None` is returned
    /// then a notification has been triggered.
    pub fn replace_notify_one(&self, value: T) -> Option<T> {
        let mut guard = self.mutex.lock();
        let replaced = guard.replace(value);
        // Dropping the guard before notifying consumers might
        // cause spurious wakeups. These are handled appropriately.
        drop(guard);
        // Only notify consumers on an edge trigger (None -> Some)
        // and not again after subsequent placements (Some -> Some)!
        if replaced.is_none() {
            self.condvar.notify_one();
        }
        replaced
    }

    /// Replace the current value and notify all waiting consumers
    ///
    /// Returns the previous value or `None`. If `None` is returned
    /// then a notification has been triggered.
    pub fn replace_notify_all(&self, value: T) -> Option<T> {
        let mut guard = self.mutex.lock();
        let replaced = guard.replace(value);
        // Dropping the guard before notifying consumers might
        // cause spurious wakeups. These are handled appropriately.
        drop(guard);
        // Only notify consumers on an edge trigger (None -> Some)
        // and not again after subsequent placements (Some -> Some)!
        if replaced.is_none() {
            self.condvar.notify_all();
        }
        replaced
    }

    /// Take the current value immediately
    ///
    /// Resets the internal state on return.
    ///
    /// Returns the previous value or `None`.
    pub fn take(&self) -> Option<T> {
        let mut guard = self.mutex.lock();
        guard.take()
    }

    /// Wait for a value and then take it
    ///
    /// Resets the internal state on return.
    ///
    /// Returns the previous value.
    pub fn wait(&self) -> T {
        let mut guard = self.mutex.lock();
        // The loop is required to handle spurious wakeups
        loop {
            if let Some(value) = guard.take() {
                return value;
            }
            self.condvar.wait(&mut guard);
        }
    }

    /// Wait for a value with a timeout and then take it
    ///
    /// Resets the internal state on return, i.e. either takes the value
    /// or on timeout the internal value already was `None` and doesn't
    /// need to be reset.
    ///
    /// Returns the value if available or `None` if the timeout expired.
    pub fn wait_for(&self, timeout: Duration) -> Option<T> {
        // Handle edge case separately
        if timeout.is_zero() {
            return self.take();
        }
        // Handling spurious timeouts in a loop would require to adjust the
        // timeout on each turn by calculating the remaining timeout from
        // the elapsed timeout! This is tedious, error prone, and could cause
        // jitter when done wrong. Better delegate this task to the
        // deadline-constrained wait function.
        if let Some(deadline) = Instant::now().checked_add(timeout) {
            self.wait_until(deadline)
        } else {
            // Wait without a deadline if the result cannot be represented
            // by an Instant
            Some(self.wait())
        }
    }

    /// Wait for a value until a deadline and then take it
    ///
    /// Resets the internal state on return, i.e. either takes the value
    /// or on timeout the internal value already was `None` and doesn't
    /// need to be reset.
    ///
    /// Returns the value if available or `None` if the deadline expired.
    pub fn wait_until(&self, deadline: Instant) -> Option<T> {
        let mut guard = self.mutex.lock();
        // The loop is required to handle spurious wakeups
        while guard.is_none() && !self.condvar.wait_until(&mut guard, deadline).timed_out() {
            continue; // Continue on spurious wakeup
        }
        guard.take()
    }
}

#[cfg(test)]
mod tests;
