use std::time::{Duration, Instant};

use thiserror::Error;

use crate::sync::{
    atomic::{
        AtomicState, AtomicU8, Ordering, SwitchAtomicStateErr, SwitchAtomicStateOk,
        SwitchAtomicStateResult,
    },
    Arc, Relay, Weak,
};

/// Desired worker progress
///
/// Non-compulsary intention or request on how the worker should
/// proceed with the pending work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressHint {
    /// Worker should continue uninterrupted
    Continue,

    /// Worker should complete the current unit of work asap
    /// with [`Completion::Suspending`].
    Suspend,

    /// Worker should complete the current unit of work asap
    /// with [`Completion::Finishing`].
    Finish,
}

impl ProgressHint {
    /// Default value
    ///
    /// The default should be used when no other information is available,
    /// i.e. processing should continue running uninterrupted.
    pub const fn default() -> Self {
        Self::Continue
    }
}

impl Default for ProgressHint {
    fn default() -> Self {
        Self::default()
    }
}

type AtomicValue = u8;

const PROGRESS_HINT_CONTINUE: AtomicValue = 0;
const PROGRESS_HINT_SUSPENDING: AtomicValue = 1;
const PROGRESS_HINT_FINISHING: AtomicValue = 2;

/// Atomic [`ProgressHint`]
#[derive(Debug)]
struct AtomicProgressHint(AtomicU8);

fn progress_hint_from_atomic_state(from: AtomicValue) -> ProgressHint {
    match from {
        PROGRESS_HINT_CONTINUE => ProgressHint::Continue,
        PROGRESS_HINT_SUSPENDING => ProgressHint::Suspend,
        PROGRESS_HINT_FINISHING => ProgressHint::Finish,
        unexpected_value => unreachable!("unexpected progress hint value: {}", unexpected_value),
    }
}

const fn progress_hint_to_atomic_state(from: ProgressHint) -> AtomicValue {
    match from {
        ProgressHint::Continue => PROGRESS_HINT_CONTINUE,
        ProgressHint::Suspend => PROGRESS_HINT_SUSPENDING,
        ProgressHint::Finish => PROGRESS_HINT_FINISHING,
    }
}

impl AtomicState for AtomicProgressHint {
    type State = ProgressHint;

    fn peek(&self) -> Self::State {
        progress_hint_from_atomic_state(self.0.load(Ordering::Relaxed))
    }

    fn load(&self) -> Self::State {
        progress_hint_from_atomic_state(self.0.load(Ordering::Acquire))
    }

    fn switch_to_desired(&self, desired_state: Self::State) -> SwitchAtomicStateOk<Self::State> {
        let desired_value = progress_hint_to_atomic_state(desired_state);
        let previous_value = self.0.swap(desired_value, Ordering::Release);
        if previous_value == desired_value {
            return SwitchAtomicStateOk::Ignored;
        }
        SwitchAtomicStateOk::Accepted {
            previous_state: progress_hint_from_atomic_state(previous_value),
        }
    }

    fn switch_from_expected_to_desired(
        &self,
        expected_state: Self::State,
        desired_state: Self::State,
    ) -> SwitchAtomicStateResult<Self::State> {
        let expected_value = progress_hint_to_atomic_state(expected_state);
        let desired_value = progress_hint_to_atomic_state(desired_state);
        self.0
            .compare_exchange(
                expected_value,
                desired_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_previous_value| {
                debug_assert_eq!(expected_value, _previous_value);
                if desired_value == expected_value {
                    SwitchAtomicStateOk::Ignored
                } else {
                    SwitchAtomicStateOk::Accepted {
                        previous_state: expected_state,
                    }
                }
            })
            .or_else(|current_value| {
                debug_assert_ne!(expected_value, current_value);
                if desired_value == current_value {
                    Ok(SwitchAtomicStateOk::Ignored)
                } else {
                    Err(SwitchAtomicStateErr::Rejected {
                        current_state: progress_hint_from_atomic_state(current_value),
                    })
                }
            })
    }
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
        Self(AtomicU8::new(progress_hint_to_atomic_state(progress_hint)))
    }

    // The loom atomic does not provide a const fn new()
    #[cfg(loom)]
    fn new(progress_hint: ProgressHint) -> Self {
        Self(AtomicU8::new(Self::to_atomic_state(progress_hint)))
    }

    /// Switch from [`ProgressHint::Continue`] to [`ProgressHint::Suspend`]
    pub fn suspend(&self) -> SwitchAtomicStateResult<ProgressHint> {
        self.switch_from_expected_to_desired(ProgressHint::Continue, ProgressHint::Suspend)
    }

    /// Switch from [`ProgressHint::Suspend`] to [`ProgressHint::Continue`]
    pub fn resume(&self) -> SwitchAtomicStateResult<ProgressHint> {
        self.switch_from_expected_to_desired(ProgressHint::Suspend, ProgressHint::Continue)
    }

    /// Switch from any state to [`ProgressHint::Finish`]
    ///
    /// Currently, finishing is permitted in any state. But this
    /// may change in the future.
    pub fn finish(&self) -> SwitchAtomicStateResult<ProgressHint> {
        Ok(self.switch_to_desired(ProgressHint::Finish))
    }

    /// Reset to [`ProgressHint::default()`]
    ///
    /// Resetting is enforced regardless of the current state and never fails.
    pub fn reset(&self) -> SwitchAtomicStateOk<ProgressHint> {
        self.switch_to_desired(ProgressHint::default())
    }
}

impl Default for AtomicProgressHint {
    fn default() -> Self {
        Self::default()
    }
}

// Zero-sized token for passing update notifications
#[derive(Debug)]
struct UpdateNotificationToken;

/// Handover of progress hint values from multiple senders to
/// a single receiver.
///
/// Allows a receiver to read the latest progress hint value
/// without blocking, i.e. lock-free under real-time constraints.
///
/// On demand a receiver may block and wait for the next update
/// notification from a sender. A progress hint update notification
/// is buffered until consumed by the receiver. Subsequent updates
/// will only trigger a single notifications.
#[derive(Debug)]
struct ProgressHintHandover {
    atomic_state: AtomicProgressHint,
    update_notification_relay: Relay<UpdateNotificationToken>,
}

impl ProgressHintHandover {
    pub const fn default() -> Self {
        Self {
            atomic_state: AtomicProgressHint::default(),
            update_notification_relay: Relay::new(),
        }
    }
}

impl Default for ProgressHintHandover {
    fn default() -> Self {
        Self::default()
    }
}

/// The observed effect of switching the progress hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchProgressHintOk {
    /// Changed as desired and signaled
    Accepted { previous_state: ProgressHint },

    /// Unchanged (i.e. already as desired) and silently ignored (i.e. not signaled)
    Ignored,
}

impl From<SwitchAtomicStateOk<ProgressHint>> for SwitchProgressHintOk {
    fn from(from: SwitchAtomicStateOk<ProgressHint>) -> Self {
        match from {
            SwitchAtomicStateOk::Accepted { previous_state } => Self::Accepted { previous_state },
            SwitchAtomicStateOk::Ignored => Self::Ignored,
        }
    }
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
    Rejected { current_state: ProgressHint },
}

impl From<SwitchAtomicStateErr<ProgressHint>> for SwitchProgressHintError {
    fn from(from: SwitchAtomicStateErr<ProgressHint>) -> Self {
        match from {
            SwitchAtomicStateErr::Rejected { current_state } => Self::Rejected { current_state },
        }
    }
}

pub type SwitchProgressHintResult = Result<SwitchProgressHintOk, SwitchProgressHintError>;

impl ProgressHintHandover {
    pub fn peek(&self) -> ProgressHint {
        self.atomic_state.peek()
    }

    pub fn load(&self) -> ProgressHint {
        self.atomic_state.load()
    }

    fn after_atomic_state_switched_result(
        &self,
        result: SwitchAtomicStateResult<ProgressHint>,
    ) -> SwitchProgressHintResult {
        result
            .map(|ok| self.after_atomic_state_switched_ok(ok))
            .map_err(Into::into)
    }

    fn after_atomic_state_switched_ok(
        &self,
        ok: SwitchAtomicStateOk<ProgressHint>,
    ) -> SwitchProgressHintOk {
        if matches!(ok, SwitchAtomicStateOk::Accepted { .. }) {
            self.update_notification_relay.replace_notify_one(UpdateNotificationToken);
        }
        ok.into()
    }

    pub fn suspend(&self) -> SwitchProgressHintResult {
        self.after_atomic_state_switched_result(self.atomic_state.suspend())
    }

    pub fn resume(&self) -> SwitchProgressHintResult {
        self.after_atomic_state_switched_result(self.atomic_state.resume())
    }

    pub fn finish(&self) -> SwitchProgressHintResult {
        self.after_atomic_state_switched_result(self.atomic_state.finish())
    }

    pub fn wait(&self) {
        self.update_notification_relay.wait();
    }

    pub fn wait_for(&self, timeout: Duration) -> bool {
        self.update_notification_relay.wait_for(timeout).is_some()
    }

    pub fn wait_until(&self, deadline: Instant) -> bool {
        self.update_notification_relay.wait_until(deadline).is_some()
    }

    pub fn wait_while_suspending(&self) -> ProgressHint {
        let mut latest_hint = self.atomic_state.load();
        while latest_hint == ProgressHint::Suspend {
            self.update_notification_relay.wait();
            latest_hint = self.atomic_state.load()
        }
        latest_hint
    }

    pub fn reset(&self) {
        self.atomic_state.reset();
        self.update_notification_relay.take();
    }

    pub fn try_suspending(&self) -> bool {
        self.atomic_state.suspend().is_ok()
        // No update notification needed nor intended!
    }

    pub fn try_finishing(&self) -> bool {
        self.atomic_state.finish().is_ok()
        // No update notification needed nor intended!
    }
}

/// Sender of the progress hint handover protocol
#[derive(Debug, Clone)]
pub struct ProgressHintSender {
    handover: Weak<ProgressHintHandover>,
}

impl ProgressHintSender {
    pub fn attach(rx: &ProgressHintReceiver) -> Self {
        let handover = Arc::downgrade(&rx.handover);
        ProgressHintSender { handover }
    }

    pub fn is_attached(&self) -> bool {
        self.handover.strong_count() > 0
    }

    fn upgrade_handover(&self) -> Result<Arc<ProgressHintHandover>, SwitchProgressHintError> {
        self.handover
            .upgrade()
            .ok_or(SwitchProgressHintError::Detached)
    }

    /// Ask the receiver to suspend while running
    pub fn suspend(&self) -> SwitchProgressHintResult {
        self.upgrade_handover()
            .and_then(|handover| handover.suspend())
    }

    /// Ask the receiver to resume while suspended
    pub fn resume(&self) -> SwitchProgressHintResult {
        self.upgrade_handover()
            .and_then(|handover| handover.resume())
    }

    /// Ask the receiver to finish
    pub fn finish(&self) -> SwitchProgressHintResult {
        self.upgrade_handover()
            .and_then(|handover| handover.finish())
    }
}

/// Receiver of the progress hint handover protocol
#[derive(Debug, Default)]
pub struct ProgressHintReceiver {
    handover: Arc<ProgressHintHandover>,
}

impl ProgressHintReceiver {
    /// Read the latest progress hint (lock-free)
    ///
    /// Reads the current value using `relaxed` semantics (memory order)
    /// and leaves any pending handover notifications untouched.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn peek(&self) -> ProgressHint {
        self.handover.peek()
    }

    /// Read the latest progress hint (lock-free)
    ///
    /// Reads the current value using `acquire` semantics (memory order)
    /// and leaves any pending handover notifications untouched.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn load(&self) -> ProgressHint {
        self.handover.load()
    }

    /// Wait for a progress hint update notification (blocking)
    ///
    /// Blocks until a handover notification is available. Use deliberately
    /// to prevent infinite blocking!
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the notification
    /// could cause a priority inversion.
    pub fn wait(&self) {
        self.handover.wait()
    }

    /// Wait for a progress hint update notification with a timeout (blocking)
    ///
    /// Blocks until a handover notification is available (`true`) or the
    /// timeout has expired (`false`).
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the notification
    /// could cause a priority inversion.
    pub fn wait_for(&self, timeout: Duration) -> bool {
        self.handover.wait_for(timeout)
    }

    /// Wait for a progress hint update notification with a deadline (blocking)
    ///
    /// Blocks until a handover notification is available (`true`) or the
    /// deadline has expired (`false`).
    ///
    /// This function might block and thus should not be invoked in
    /// a hard real-time context! The sending threads of the notification
    /// could cause a priority inversion.
    pub fn wait_until(&self, deadline: Instant) -> bool {
        self.handover.wait_until(deadline)
    }

    /// Reserved for internal usage
    ///
    /// Silently try to switch to [`ProgressHint::Suspend`] (lock-free).
    ///
    /// Intentionally declared as &mut to make it inaccessible for
    /// borrowed references!
    pub fn try_suspending(&mut self) -> bool {
        self.handover.try_suspending()
    }

    /// Reserved for internal usage
    ///
    /// Park the thread during [`ProgressHint::Suspend`] (blocking).
    ///
    /// Intentionally declared as &mut to make it inaccessible for
    /// borrowed references!
    pub fn wait_while_suspending(&mut self) -> ProgressHint {
        self.handover.wait_while_suspending()
    }

    /// Reserved for internal usage
    ///
    /// Silently try to switch to [`ProgressHint::Finish`] (lock-free).
    ///
    /// Intentionally declared as &mut to make it inaccessible for
    /// borrowed references!
    pub fn try_finishing(&mut self) -> bool {
        self.handover.try_finishing()
    }

    /// Reserved for internal usage
    ///
    /// Reset the handover (blocking).
    ///
    /// Intentionally declared as &mut to make it inaccessible for
    /// borrowed references!
    pub fn reset(&mut self) {
        self.handover.reset();
    }

    /// Reserved for internal usage
    ///
    /// Detach all senders (lock-free). This will also reset the
    /// handover back to default.
    ///
    /// This function does not block and thus could be invoked
    /// safely in a real-time context.
    pub fn detach(&mut self) {
        self.handover = Default::default();
    }
}

#[cfg(test)]
mod tests;
