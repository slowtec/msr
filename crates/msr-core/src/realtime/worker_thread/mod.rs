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
    Terminating,
    Stopping,
}

pub trait Notifications {
    fn notify_state_changed(&mut self, state: State);
}

pub type NotificationsBoxed = Box<dyn Notifications + Send + 'static>;

impl Notifications for NotificationsBoxed {
    fn notify_state_changed(&mut self, state: State) {
        (&mut **self).notify_state_changed(state)
    }
}

/// Spawn parameters
///
/// The parameters are passed into the worker thread when spawned
/// and are recovered after joining the worker thread for later reuse.
///
/// If joining the work thread fails these parameters will be lost
/// inevitably!
#[derive(Debug)]
pub struct RecoverableParams<W, E, N> {
    pub worker: W,
    pub environment: E,
    pub notifications: N,
}

#[derive(Debug)]
pub struct WorkerThread<W, E, N> {
    join_handle: JoinHandle<TerminatedThread<W, E, N>>,
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

fn thread_fn<W: Worker, N: Notifications>(
    progress_hint_rx: &mut ProgressHintReceiver,
    mut recoverable_params: &mut RecoverableParams<W, <W as Worker>::Environment, N>,
) -> Result<()> {
    let RecoverableParams {
        worker,
        environment,
        notifications,
    } = &mut recoverable_params;

    log::info!("Starting");
    notifications.notify_state_changed(State::Starting);

    worker.start_working(environment)?;
    loop {
        log::info!("Running");
        notifications.notify_state_changed(State::Running);
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
                notifications.notify_state_changed(State::Suspending);
                progress_hint_rx.wait_for_signal_while_suspending();
            }
            Completion::Terminating => {
                // The worker may have decided to terminate itself independent
                // of the current progress hint. Termination cannot be rejected.
                progress_hint_rx.on_terminating();
                log::debug!("Terminating");
                notifications.notify_state_changed(State::Terminating);
                worker.finish_working(environment)?;
                // Exit loop
                break;
            }
        }
    }

    log::info!("Stopping");
    notifications.notify_state_changed(State::Stopping);

    Ok(())
}

/// Outcome of [`Thread::join()`]
#[derive(Debug)]
pub struct TerminatedThread<W, E, N> {
    /// The result of the thread function
    pub result: Result<()>,

    /// The recovered parameters
    pub recovered_params: RecoverableParams<W, E, N>,
}

/// Outcome of [`Thread::join()`]
#[derive(Debug)]
pub enum JoinedThread<W, E, N> {
    Terminated(TerminatedThread<W, E, N>),
    JoinError(Box<dyn Any + Send + 'static>),
}

impl<W, E, N> WorkerThread<W, E, N>
where
    W: Worker<Environment = E> + Send + 'static,
    E: Send + 'static,
    N: Notifications + Send + 'static,
{
    pub fn spawn(
        progress_hint_rx: ProgressHintReceiver,
        recoverable_params: RecoverableParams<W, E, N>,
    ) -> Self {
        let join_handle = {
            std::thread::spawn({
                move || {
                    adjust_current_thread_priority();
                    // The function parameters need to be mutable within the real-time thread
                    let mut progress_hint_rx = progress_hint_rx;
                    let mut recoverable_params = recoverable_params;
                    let result = thread_fn(&mut progress_hint_rx, &mut recoverable_params);
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

    pub fn join(self) -> JoinedThread<W, E, N> {
        let Self { join_handle } = self;
        join_handle
            .join()
            .map(JoinedThread::Terminated)
            .unwrap_or_else(JoinedThread::JoinError)
    }
}
