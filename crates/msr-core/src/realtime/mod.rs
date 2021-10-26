use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::Duration,
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

#[derive(Debug, Clone)]
pub struct ProgressHintSender {
    latest_progress_hint: Arc<AtomicProgressHint>,
    handshake_tx: mpsc::SyncSender<()>,
}

impl ProgressHintSender {
    pub fn suspend(&self) -> bool {
        if !self.latest_progress_hint.suspend() {
            return false;
        }
        let _ = self.handshake_tx.try_send(());
        true
    }

    pub fn resume(&self) -> bool {
        if !self.latest_progress_hint.resume() {
            return false;
        }
        let _ = self.handshake_tx.try_send(());
        true
    }

    pub fn terminate(&self) {
        self.latest_progress_hint.terminate();
        let _ = self.handshake_tx.try_send(());
    }
}

#[derive(Debug)]
pub struct ProgressHintReceiver {
    latest_progress_hint: Arc<AtomicProgressHint>,
    handshake_rx: mpsc::Receiver<()>,
}

impl ProgressHintReceiver {
    /// Load the latest value
    ///
    /// Leave any pending handshake signals untouched.
    pub fn load(&self) -> ProgressHint {
        self.latest_progress_hint.load()
    }

    /// Read the latest progress hint without blocking
    ///
    /// Clears any pending handshake signals before reading
    /// the latest value.
    pub fn recv(&self) -> ProgressHint {
        let _ = self.handshake_rx.try_recv();
        self.load()
    }

    /// Receive a new progress hint with blocking
    ///
    /// Blocks until a new handshake signal has been received
    /// or the timeout has expired and then reads the latest
    /// value.
    pub fn recv_timeout(&self, timeout: Duration) -> ProgressHint {
        let _ = self.handshake_rx.recv_timeout(timeout);
        self.load()
    }

    // Receive a new progress hint with blocking
    //
    // Blocks until a new handshake signal has been received
    // or the deadline has expired and then reads the latest
    // value.
    //
    // TODO: Enable when available
    // https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.recv_deadline
    // pub fn recv_deadline(&self, deadline: Instant) -> ProgressHint {
    //     let _ = self.handshake_rx.recv_deadline(deadline);
    //     self.load()
    // }
}

pub(crate) fn new_progress_hint_channel() -> (ProgressHintSender, ProgressHintReceiver) {
    let (handshake_tx, handshake_rx) = mpsc::sync_channel(1);
    let latest_progress_hint = Arc::new(AtomicProgressHint::default());
    let progress_hint_tx = ProgressHintSender {
        latest_progress_hint: latest_progress_hint.clone(),
        handshake_tx,
    };
    let progress_hint_rx = ProgressHintReceiver {
        latest_progress_hint,
        handshake_rx,
    };
    (progress_hint_tx, progress_hint_rx)
}

#[cfg(test)]
mod tests;
