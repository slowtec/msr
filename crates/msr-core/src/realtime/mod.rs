use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

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
struct AtomicProgressHint(AtomicU8);

impl AtomicProgressHint {
    /// Default value
    ///
    /// Creates a new value in accordance to `ProgressHint::default()`.
    pub const fn default() -> Self {
        Self(AtomicU8::new(PROGRESS_HINT_RUNNING))
    }

    fn switch_from_current_to_desired(&self, current: u8, desired: u8) -> bool {
        match self
            .0
            .compare_exchange(current, desired, Ordering::Acquire, Ordering::Acquire)
        {
            Ok(_previous) => true,
            Err(current) => current == desired,
        }
    }

    /// Switch from [`ProgressHint::Running`] to [`ProgressHint::Suspending`]
    ///
    /// Returns `true` if successful or already suspending and `false` otherwise.
    pub fn suspend(&self) -> bool {
        self.switch_from_current_to_desired(PROGRESS_HINT_RUNNING, PROGRESS_HINT_SUSPENDING)
    }

    /// Switch from [`ProgressHint::Suspending`] to [`ProgressHint::Running`]
    ///
    /// Returns `true` if successful or already running and `false` otherwise.
    pub fn resume(&self) -> bool {
        self.switch_from_current_to_desired(PROGRESS_HINT_SUSPENDING, PROGRESS_HINT_RUNNING)
    }

    /// Reset to [`ProgressHint::default()`]
    ///
    /// Returns `true` if successful and `false` otherwise.
    #[allow(dead_code)]
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

#[derive(Debug, Default)]
#[allow(clippy::mutex_atomic)]
struct ProgressHintHandshake {
    atomic: AtomicProgressHint,
    signal_mutex: Mutex<bool>,
    signal_condvar: Condvar,
}

#[allow(clippy::mutex_atomic)]
impl ProgressHintHandshake {
    pub fn load(&self) -> ProgressHint {
        self.atomic.load()
    }

    fn signal_one(&self) -> anyhow::Result<()> {
        let mut signal_guard = self
            .signal_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("signal poisened"))?;
        *signal_guard = true;
        self.signal_condvar.notify_one();
        Ok(())
    }

    pub fn clear_signal(&self) -> anyhow::Result<()> {
        let mut signal_guard = self
            .signal_mutex
            .lock()
            .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
        *signal_guard = false;
        Ok(())
    }

    pub fn suspend(&self) -> anyhow::Result<bool> {
        if !self.atomic.suspend() {
            return Ok(false);
        }
        self.signal_one()?;
        Ok(true)
    }

    pub fn resume(&self) -> anyhow::Result<bool> {
        if !self.atomic.resume() {
            return Ok(false);
        }
        self.signal_one()?;
        Ok(true)
    }

    pub fn terminate(&self) -> anyhow::Result<()> {
        self.atomic.terminate();
        self.signal_one()?;
        Ok(())
    }

    pub fn wait_timeout(&self, timeout: Duration) -> anyhow::Result<()> {
        if !timeout.is_zero() {
            let signal_guard = self
                .signal_mutex
                .lock()
                .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
            let _wait_result = self
                .signal_condvar
                .wait_timeout_while(signal_guard, timeout, |signal| {
                    if *signal {
                        // Clear signal and abort waiting
                        *signal = false;
                        return false;
                    }
                    // Continue waiting
                    true
                })
                .map_err(|_| anyhow::anyhow!("condvar/mutex poisened"))?;
        }
        Ok(())
    }

    pub fn wait_deadline(&self, deadline: Instant) -> anyhow::Result<()> {
        let now = Instant::now();
        let timeout = deadline.duration_since(deadline.min(now));
        self.wait_timeout(timeout)
    }
}

#[derive(Debug, Clone)]
pub struct ProgressHintSender {
    handshake: Arc<ProgressHintHandshake>,
}

impl ProgressHintSender {
    pub fn suspend(&self) -> anyhow::Result<bool> {
        self.handshake.suspend()
    }

    pub fn resume(&self) -> anyhow::Result<bool> {
        self.handshake.resume()
    }

    pub fn terminate(&self) -> anyhow::Result<()> {
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

    /// Load the latest value
    ///
    /// Leave any pending handshake signals untouched.
    ///
    /// This function does not block and thus could be
    /// invoked safely in a real-time context.
    pub fn load(&self) -> ProgressHint {
        self.handshake.load()
    }

    /// Receive the latest progress hint
    ///
    /// Clears any pending handshake signals before reading
    ///
    /// This function might block and thus should not be
    /// invoked in a real-time context!
    pub fn recv_clear(&self) -> ProgressHint {
        let _ = self.handshake.clear_signal();
        self.load()
    }

    /// Receive the latest progress hint, waiting for a signal
    ///
    /// Blocks until a new handshake signal has been received
    /// or the timeout has expired and then reads the latest
    /// value.
    ///
    /// This function might block and thus should not be
    /// invoked in a real-time context!
    pub fn recv_timeout(&self, timeout: Duration) -> ProgressHint {
        let _ = self.handshake.wait_timeout(timeout);
        self.load()
    }

    /// Receive the latest progress hint, waiting for a signal
    ///
    /// Blocks until a new handshake signal has been received
    /// or the deadline has expired and then reads the latest
    /// value.
    ///
    /// This function might block and thus should not be
    /// invoked in a real-time context!
    pub fn recv_deadline(&self, deadline: Instant) -> ProgressHint {
        let _ = self.handshake.wait_deadline(deadline);
        self.load()
    }
}

#[cfg(test)]
mod tests;
