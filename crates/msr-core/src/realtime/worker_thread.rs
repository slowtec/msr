use std::{any::Any, thread::JoinHandle};

use anyhow::Result;
use thread_priority::ThreadPriority;

use super::processing::{
    processor::{Processor, Progress},
    progresshint::{ProgressHintReceiver, ProgressHintSender},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Running,
    Suspended,
    Finishing,
    Terminating,
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
pub struct Params<E, N, P> {
    pub environment: E,
    pub notifications: N,
    pub processor: P,
}

#[derive(Debug)]
pub struct Thread<E, N, P> {
    progress_hint_tx: ProgressHintSender,
    join_handle: JoinHandle<TerminatedThread<E, N, P>>,
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

fn thread_fn<N: Notifications, E, P: Processor<E>>(
    progress_hint_rx: &ProgressHintReceiver,
    mut params: &mut Params<E, N, P>,
) -> Result<()> {
    let Params {
        environment,
        notifications,
        processor,
    } = &mut params;

    log::info!("Starting");
    notifications.notify_state_changed(State::Starting);

    processor.start_processing(environment)?;

    loop {
        log::info!("Running");
        notifications.notify_state_changed(State::Running);
        match processor.process(environment, progress_hint_rx)? {
            Progress::Suspended => {
                // Try to suspend ourselves
                let suspended = progress_hint_rx.suspend();
                if suspended.is_err() {
                    // Might have been terminated
                    continue;
                }
                notifications.notify_state_changed(State::Suspended);
                progress_hint_rx.wait_for_signal_while_suspending();
            }
            Progress::Terminated => {
                log::debug!("Processing terminated");
                // Exit loop
                break;
            }
        }
    }

    log::info!("Finishing");
    notifications.notify_state_changed(State::Finishing);

    processor.finish_processing(environment)?;

    log::info!("Terminating");
    notifications.notify_state_changed(State::Terminating);

    Ok(())
}

/// Outcome of [`Thread::join()`]
#[derive(Debug)]
pub struct TerminatedThread<E, N, P> {
    /// The result of the thread function
    pub result: Result<()>,

    /// The recovered parameters
    pub recovered_params: Params<E, N, P>,
}

/// Outcome of [`Thread::join()`]
#[derive(Debug)]
pub enum JoinedThread<E, N, P> {
    Terminated(TerminatedThread<E, N, P>),
    JoinError(Box<dyn Any + Send + 'static>),
}

impl<E, N, P> Thread<E, N, P>
where
    E: Send + 'static,
    N: Notifications + Send + 'static,
    P: Processor<E> + Send + 'static,
{
    pub fn spawn(params: Params<E, N, P>) -> Self {
        let progress_hint_rx = ProgressHintReceiver::default();
        let progress_hint_tx = ProgressHintSender::attach(&progress_hint_rx);
        let join_handle = {
            std::thread::spawn({
                move || {
                    adjust_current_thread_priority();
                    // The parameters are mutable within the real-time thread
                    let mut params = params;
                    let result = thread_fn(&progress_hint_rx, &mut params);
                    let recovered_params = params;
                    TerminatedThread {
                        result,
                        recovered_params,
                    }
                }
            })
        };
        Self {
            progress_hint_tx,
            join_handle,
        }
    }

    pub fn progress_hint_sender(&self) -> &ProgressHintSender {
        &self.progress_hint_tx
    }

    pub fn join(self) -> JoinedThread<E, N, P> {
        let Self {
            progress_hint_tx: _,
            join_handle,
        } = self;
        join_handle
            .join()
            .map(JoinedThread::Terminated)
            .unwrap_or_else(JoinedThread::JoinError)
    }
}
