use std::{
    any::Any,
    thread::{self, JoinHandle},
};

use anyhow::Result;
use thread_priority::{ThreadId as NativeThreadId, ThreadPriority, ThreadSchedulePolicy};

use super::{progress::ProgressHintReceiver, CompletionStatus, Worker};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, num_derive::FromPrimitive)]
#[repr(u8)]
pub enum State {
    #[default]
    Unknown,
    Starting,
    Started,
    Running,
    Suspended,
    Resumed,
    Finishing,
    Finished,
    Terminating,
}

impl State {
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        num_traits::FromPrimitive::from_u8(value)
    }
}

/// Emitted event
#[derive(Debug)]
pub enum Event {
    /// The new state
    StateChanged(State),
}

pub trait EmitEvent {
    fn emit_event(&mut self, event: Event);
}

/// Spawn parameters
///
/// The parameters are passed into the worker thread when spawned
/// and are recovered after joining the worker thread for later reuse.
///
/// If joining the work thread fails these parameters will be lost
/// inevitably!
#[allow(missing_debug_implementations)]
pub struct Context<W: Worker, E: EmitEvent> {
    pub progress_hint_rx: ProgressHintReceiver,
    pub worker: W,
    pub environment: <W as Worker>::Environment,
    pub emit_event: E,
}

#[derive(Debug)]
pub struct WorkerThread<W: Worker, E: EmitEvent> {
    join_handle: JoinHandle<TerminatedThread<W, E>>,
}

struct ThreadSchedulingScope {
    native_id: NativeThreadId,
    saved_priority: ThreadPriority,

    #[cfg(target_os = "linux")]
    saved_policy: ThreadSchedulePolicy,
}

// TODO: Prevent passing of instances to different threads
//#![feature(negative_impls)]
//impl !Send for ThreadSchedulingScope {}

impl ThreadSchedulingScope {
    #[cfg(target_os = "linux")]
    fn enter() -> anyhow::Result<Self> {
        log::debug!("Entering real-time scope");
        let native_id = thread_priority::thread_native_id();
        let thread_id = thread::current().id();
        let saved_policy = thread_priority::unix::thread_schedule_policy().map_err(|err| {
            anyhow::anyhow!(
                "Failed to save the thread scheduling policy of the current process: {:?}",
                err,
            )
        })?;
        let saved_priority =
            thread_priority::unix::get_thread_priority(native_id).map_err(|err| {
                anyhow::anyhow!(
                    "Failed to save the priority of thread {:?} ({:?}): {:?}",
                    thread_id,
                    native_id,
                    err,
                )
            })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({:?}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_priority,
                adjusted_priority
            );
        }
        let adjusted_policy = thread_priority::unix::ThreadSchedulePolicy::Realtime(
            // Non-preemptive scheduling (in contrast to RoundRobin)
            thread_priority::unix::RealtimeThreadSchedulePolicy::Fifo,
        );
        if adjusted_policy != saved_policy {
            log::debug!(
                "Adjusting scheduling policy of thread {:?} ({:?}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_policy,
                adjusted_policy
            );
        }
        if let Err(err) = thread_priority::unix::set_thread_priority_and_policy(
            native_id,
            adjusted_priority,
            adjusted_policy,
        ) {
            log::warn!(
                "Failed to adjust priority and scheduling policy of thread {:?} ({:?}): {:?}",
                thread_id,
                native_id,
                err
            );
            // Fallback: Only try to adjust the priority
            thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
                anyhow::anyhow!(
                    "Failed to adjust priority of thread {:?} ({:?}): {:?}",
                    thread_id,
                    native_id,
                    err
                )
            })?;
        }
        Ok(Self {
            native_id,
            saved_policy,
            saved_priority,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn enter() -> anyhow::Result<Self> {
        log::debug!("Entering real-time scope");
        let native_id = thread_priority::thread_native_id();
        let thread_id = thread::current().id();
        let saved_priority = thread_priority::unix::thread_priority().map_err(|err| {
            anyhow::anyhow!(
                "Failed to save the priority of thread {:?} ({:?}): {:?}",
                thread_id,
                native_id,
                err,
            )
        })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({:?}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_priority,
                adjusted_priority
            );
        }
        thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
            anyhow::anyhow!(
                "Failed to adjust priority of thread {:?} ({:?}): {:?}",
                thread_id,
                native_id,
                err
            )
        })?;
        Ok(Self {
            native_id,
            saved_priority,
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn maximize_current_thread_priority() -> anyhow::Result<(NativeThreadId, ThreadPriority)> {
        let native_id = thread_priority::thread_native_id();
        let thread_id = thread::current().id();
        let saved_priority = thread_priority::unix::thread_priority().map_err(|err| {
            anyhow::anyhow!(
                "Failed to save the priority of thread {:?} ({:?}): {:?}",
                thread_id,
                native_id,
                err,
            )
        })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({:?}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_priority,
                adjusted_priority
            );
        }
        thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
            anyhow::anyhow!(
                "Failed to adjust priority of thread {:?} ({:?}): {:?}",
                thread_id,
                native_id,
                err
            )
        })?;
        Ok((native_id, saved_priority))
    }
    #[cfg(not(target_os = "linux"))]
    pub fn enter() -> anyhow::Result<Self> {
        log::debug!("Entering real-time scope");
        let (native_id, saved_priority) = Self::maximize_current_thread_priority()?;
        Ok(Self {
            native_id,
            saved_priority,
        })
    }
}

impl Drop for ThreadSchedulingScope {
    #[cfg(target_os = "linux")]
    fn drop(&mut self) {
        log::debug!("Leaving real-time scope");
        assert_eq!(self.native_id, thread_priority::thread_native_id());
        if let Err(err) = thread_priority::unix::set_thread_priority_and_policy(
            self.native_id,
            self.saved_priority,
            self.saved_policy,
        ) {
            log::error!(
                "Failed to restore priority and scheduling policy of thread {:?} ({:?}): {:?}",
                thread::current().id(),
                self.native_id,
                err
            )
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn drop(&mut self) {
        log::debug!("Leaving real-time scope");
        assert_eq!(self.native_id, thread_priority::thread_native_id());
        if let Err(err) = thread_priority::set_current_thread_priority(self.saved_priority) {
            log::error!(
                "Failed to restore priority of thread {:?} ({:?}): {:?}",
                thread::current().id(),
                self.native_id,
                err
            )
        }
    }
}

fn thread_fn<W, E>(thread_scheduling: ThreadScheduling, context: &mut Context<W, E>) -> Result<()>
where
    W: Worker,
    E: EmitEvent,
{
    let Context {
        progress_hint_rx,
        worker,
        environment,
        emit_event,
    } = context;

    log::debug!("Starting");
    emit_event.emit_event(Event::StateChanged(State::Starting));

    worker.start_working(environment)?;

    log::debug!("Started");
    emit_event.emit_event(Event::StateChanged(State::Started));

    let scheduling_scope = match thread_scheduling {
        ThreadScheduling::Default => None,
        ThreadScheduling::Realtime => Some(ThreadSchedulingScope::enter()?),
        ThreadScheduling::RealtimeOrDefault => ThreadSchedulingScope::enter().ok(),
    };
    loop {
        log::debug!("Running");
        emit_event.emit_event(Event::StateChanged(State::Running));

        match worker.perform_work(environment, progress_hint_rx)? {
            CompletionStatus::Suspending => {
                // The worker may have decided to suspend itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_suspending() {
                    // Suspending is not permitted
                    log::debug!("Suspending rejected");
                    continue;
                }

                log::debug!("Suspended");
                emit_event.emit_event(Event::StateChanged(State::Suspended));

                progress_hint_rx.wait_while_suspending();

                log::debug!("Resumed");
                emit_event.emit_event(Event::StateChanged(State::Resumed));
            }
            CompletionStatus::Finishing => {
                // The worker may have decided to finish itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_finishing() {
                    // Suspending is not permitted
                    log::debug!("Finishing rejected");
                    continue;
                }
                // Leave custom scheduling scope before finishing
                drop(scheduling_scope);
                // Exit running loop
                break;
            }
        }
    }

    log::debug!("Finishing");
    emit_event.emit_event(Event::StateChanged(State::Finishing));

    worker.finish_working(environment)?;

    log::debug!("Finished");
    emit_event.emit_event(Event::StateChanged(State::Finished));

    log::debug!("Terminating");
    emit_event.emit_event(Event::StateChanged(State::Terminating));

    Ok(())
}

/// Outcome of [`WorkerThread::join()`]
#[allow(missing_debug_implementations)]
pub struct TerminatedThread<W: Worker, E: EmitEvent> {
    /// The result of the thread function
    pub result: Result<()>,

    /// The recovered parameters
    pub context: Context<W, E>,
}

/// Outcome of [`WorkerThread::join()`]
#[allow(missing_debug_implementations)]
pub enum JoinedThread<W: Worker, E: EmitEvent> {
    Terminated(TerminatedThread<W, E>),
    JoinError(Box<dyn Any + Send + 'static>),
}

#[derive(Debug, Clone, Copy)]
pub enum ThreadScheduling {
    /// Default
    ///
    /// Do not modify the current thread's priority and leave the
    /// process's scheduling policy untouched.
    Default,

    /// Real-time
    ///
    /// Switch thread to real-time priority and try to switch to a real-time
    /// scheduling policy. The latter is optional and failures are only logged,
    /// not reported.
    Realtime,

    /// Real-time with fallback
    ///
    /// Try to apply a real-time strategy, but silently fall back `Default`
    /// if it fails. This is handy for tests in an environment that does not
    /// permit real-time scheduling, e.g. running the tests in containers
    /// on a CI platform.
    RealtimeOrDefault,
}

impl<W, E> WorkerThread<W, E>
where
    W: Worker + Send + 'static,
    <W as Worker>::Environment: Send + 'static,
    E: EmitEvent + Send + 'static,
{
    pub fn spawn(thread_scheduling: ThreadScheduling, context: Context<W, E>) -> Self {
        let join_handle = {
            std::thread::spawn({
                move || {
                    // The function parameters need to be mutable within the real-time thread
                    let mut context = context;
                    let result = thread_fn(thread_scheduling, &mut context);
                    let context = context;
                    TerminatedThread { result, context }
                }
            })
        };
        Self { join_handle }
    }

    pub fn join(self) -> JoinedThread<W, E> {
        let Self { join_handle } = self;
        join_handle
            .join()
            .map(JoinedThread::Terminated)
            .unwrap_or_else(JoinedThread::JoinError)
    }
}

#[cfg(test)]
mod tests;
