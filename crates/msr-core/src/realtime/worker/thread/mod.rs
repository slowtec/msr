use std::{
    any::Any,
    thread::{self, JoinHandle},
};

use anyhow::Result;
use thread_priority::{ThreadId as NativeThreadId, ThreadPriority, ThreadSchedulePolicy};

use super::{progress::ProgressHintReceiver, CompletionStatus, Worker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Running,
    Suspending,
    Finishing,
    Stopping,
}

/// Event callbacks
pub trait Events {
    fn on_state_changed(&mut self, state: State);
}

pub type EventsBoxed = Box<dyn Events + Send + 'static>;

impl Events for EventsBoxed {
    fn on_state_changed(&mut self, state: State) {
        (&mut **self).on_state_changed(state)
    }
}

/// Spawn parameters
///
/// The parameters are passed into the worker thread when spawned
/// and are recovered after joining the worker thread for later reuse.
///
/// If joining the work thread fails these parameters will be lost
/// inevitably!
#[allow(missing_debug_implementations)]
pub struct RecoverableParams<W: Worker, E> {
    pub progress_hint_rx: ProgressHintReceiver,
    pub worker: W,
    pub environment: <W as Worker>::Environment,
    pub events: E,
}

#[derive(Debug)]
pub struct WorkerThread<W: Worker, E> {
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
    pub fn enter() -> anyhow::Result<Self> {
        log::debug!("Entering real-time scope");
        let native_id = thread_priority::thread_native_id();
        let thread_id = thread::current().id();
        let saved_policy = thread_priority::unix::thread_schedule_policy().map_err(|err| {
            anyhow::anyhow!(
                "Failed to save the thread scheduling policy of the current process: {:?}",
                err,
            )
        })?;
        let saved_priority = thread_priority::unix::thread_priority().map_err(|err| {
            anyhow::anyhow!(
                "Failed to save the priority of thread {:?} ({}): {:?}",
                thread_id,
                native_id,
                err,
            )
        })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({}): {:?} -> {:?}",
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
                "Adjusting scheduling policy of thread {:?} ({}): {:?} -> {:?}",
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
                "Failed to adjust priority and scheduling policy of thread {:?} ({}): {:?}",
                thread_id,
                native_id,
                err
            );
            // Fallback: Only try to adjust the priority
            thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
                anyhow::anyhow!(
                    "Failed to adjust priority of thread {:?} ({}): {:?}",
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
                "Failed to save the priority of thread {:?} ({}): {:?}",
                thread_id,
                native_id,
                err,
            )
        })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_priority,
                adjusted_priority
            );
        }
        thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
            anyhow::anyhow!(
                "Failed to adjust priority of thread {:?} ({}): {:?}",
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
                "Failed to save the priority of thread {:?} ({}): {:?}",
                thread_id,
                native_id,
                err,
            )
        })?;
        let adjusted_priority = ThreadPriority::Max;
        if adjusted_priority != saved_priority {
            log::debug!(
                "Adjusting priority of thread {:?} ({}): {:?} -> {:?}",
                thread_id,
                native_id,
                saved_priority,
                adjusted_priority
            );
        }
        thread_priority::set_current_thread_priority(adjusted_priority).map_err(|err| {
            anyhow::anyhow!(
                "Failed to adjust priority of thread {:?} ({}): {:?}",
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
                "Failed to restore priority and scheduling policy of thread {:?} ({}): {:?}",
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
                "Failed to restore priority of thread {:?} ({}): {:?}",
                thread::current().id(),
                self.native_id,
                err
            )
        }
    }
}

fn thread_fn<W: Worker, E: Events>(recoverable_params: &mut RecoverableParams<W, E>) -> Result<()> {
    let RecoverableParams {
        progress_hint_rx,
        worker,
        environment,
        events,
    } = recoverable_params;

    log::debug!("Starting");
    events.on_state_changed(State::Starting);

    worker.start_task_of_work(environment)?;

    let rt_sched_scope = ThreadSchedulingScope::enter()?;
    loop {
        log::debug!("Running");
        events.on_state_changed(State::Running);
        match worker.perform_unit_of_work(environment, progress_hint_rx)? {
            CompletionStatus::Suspending => {
                // The worker may have decided to suspend itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_suspending() {
                    // Suspending is not permitted
                    log::debug!("Suspending rejected");
                    continue;
                }
                log::debug!("Suspending");
                events.on_state_changed(State::Suspending);
                progress_hint_rx.wait_while_suspending();
            }
            CompletionStatus::Finishing => {
                // The worker may have decided to finish itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_finishing() {
                    // Suspending is not permitted
                    log::debug!("Finishing rejected");
                    continue;
                }
                // Leave real-time scheduling scope
                drop(rt_sched_scope);
                log::debug!("Finishing");
                events.on_state_changed(State::Finishing);
                worker.finish_task_of_work(environment)?;
                // Exit loop
                break;
            }
        }
    }

    log::debug!("Stopping");
    events.on_state_changed(State::Stopping);

    Ok(())
}

/// Outcome of [`Thread::join()`]
#[allow(missing_debug_implementations)]
pub struct TerminatedThread<W: Worker, E> {
    /// The result of the thread function
    pub result: Result<()>,

    /// The recovered parameters
    pub recovered_params: RecoverableParams<W, E>,
}

/// Outcome of [`Thread::join()`]
#[allow(missing_debug_implementations)]
pub enum JoinedThread<W: Worker, E> {
    Terminated(TerminatedThread<W, E>),
    JoinError(Box<dyn Any + Send + 'static>),
}

impl<W, E> WorkerThread<W, E>
where
    W: Worker + Send + 'static,
    <W as Worker>::Environment: Send + 'static,
    E: Events + Send + 'static,
{
    pub fn spawn(recoverable_params: RecoverableParams<W, E>) -> Self {
        let join_handle = {
            std::thread::spawn({
                move || {
                    // The function parameters need to be mutable within the real-time thread
                    let mut recoverable_params = recoverable_params;
                    let result = thread_fn(&mut recoverable_params);
                    let recovered_params = recoverable_params;
                    TerminatedThread {
                        result,
                        recovered_params,
                    }
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
