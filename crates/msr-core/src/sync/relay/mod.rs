use std::time::{Duration, Instant};

use crate::sync::{const_mutex, Condvar, Mutex};

/// Move single values between threads
///
/// A condition variable with a memory that allows to pass single
/// values from producer to consumer threads.
///
/// A typical scenario involves only a single producer and a single
/// consumer thread implementing a handshake protocol for passing
/// the latest (= most recent) value between each other.
///
/// The value is buffered until the consumer is ready to take it.
/// Each value can be consumed at most once.
#[derive(Debug)]
pub struct Relay<T> {
    mutex: Mutex<Option<T>>,
    condvar: Condvar,
}

impl<T> Relay<T> {
    pub const fn default() -> Self {
        Self {
            mutex: const_mutex(None),
            condvar: Condvar::new(),
        }
    }
}

impl<T> Default for Relay<T> {
    fn default() -> Self {
        Self::default()
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
    /// Returns the previous value or `None`.
    pub fn take(&self) -> Option<T> {
        let mut guard = self.mutex.lock();
        guard.take()
    }

    /// Wait for a value and then take it
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
    /// Returns the value if available or `None` if the timeout expired.
    pub fn wait_for(&self, timeout: Duration) -> Option<T> {
        let mut guard = self.mutex.lock();
        // The loop is required to handle spurious wakeups
        while guard.is_none() && !self.condvar.wait_for(&mut guard, timeout).timed_out() {}
        guard.take()
    }

    /// Wait for a value until a deadline and then take it
    ///
    /// Returns the value if available or `None` if the deadline expired.
    pub fn wait_until(&self, deadline: Instant) -> Option<T> {
        let mut guard = self.mutex.lock();
        // The loop is required to handle spurious wakeups
        while guard.is_none() && !self.condvar.wait_until(&mut guard, deadline).timed_out() {}
        guard.take()
    }
}

#[cfg(test)]
mod tests;
