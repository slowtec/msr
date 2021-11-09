use std::{any::Any, thread::JoinHandle};

use anyhow::Result;
use thread_priority::ThreadPriority;

use super::{
    progress_hint::ProgressHintReceiver,
    worker::{Completion, Worker},
};

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

/// TODO: Realtime scheduling has only been confirmed to work on Linux
#[cfg(target_os = "linux")]
pub fn adjust_current_thread_priority() {
    let thread_id = thread_priority::unix::thread_native_id();
    if let Err(err) = thread_priority::unix::set_thread_priority_and_policy(
        thread_id,
        ThreadPriority::Max,
        thread_priority::unix::ThreadSchedulePolicy::Realtime(
            // Non-preemptive scheduling (in contrast to RoundRobin)
            thread_priority::unix::RealtimeThreadSchedulePolicy::Fifo,
        ),
    ) {
        log::error!(
            "Failed to adjust real-time thread priority and policy: {:?}",
            err
        );
        // Fallback: Only maximize the priority
        if let Err(err) = thread_priority::set_current_thread_priority(ThreadPriority::Max) {
            log::error!("Failed to adjust thread priority: {:?}", err);
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn adjust_current_thread_priority() {
    if let Err(err) = thread_priority::set_current_thread_priority(ThreadPriority::Max) {
        log::error!("Failed to adjust thread priority: {:?}", err);
    }
}

fn thread_fn<W: Worker, E: Events>(recoverable_params: &mut RecoverableParams<W, E>) -> Result<()> {
    let RecoverableParams {
        progress_hint_rx,
        worker,
        environment,
        events,
    } = recoverable_params;

    log::info!("Starting");
    events.on_state_changed(State::Starting);

    worker.start_working_task(environment)?;
    loop {
        log::info!("Running");
        events.on_state_changed(State::Running);
        match worker.perform_work(environment, progress_hint_rx)? {
            Completion::Suspending => {
                // The worker may have decided to suspend itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_suspending() {
                    // Suspending is not permitted
                    log::debug!("Suspending rejected");
                    continue;
                }
                log::debug!("Suspending");
                events.on_state_changed(State::Suspending);
                progress_hint_rx.wait_for_signal_while_suspending();
            }
            Completion::Finishing => {
                // The worker may have decided to finish itself independent
                // of the current progress hint.
                if !progress_hint_rx.try_finishing() {
                    // Suspending is not permitted
                    log::debug!("Finishing rejected");
                    continue;
                }
                log::debug!("Finishing");
                events.on_state_changed(State::Finishing);
                worker.finish_working_task(environment)?;
                // Exit loop
                break;
            }
        }
    }

    log::info!("Stopping");
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
                    adjust_current_thread_priority();
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
