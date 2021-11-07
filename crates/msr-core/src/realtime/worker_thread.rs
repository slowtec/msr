use std::{any::Any, thread::JoinHandle};

use anyhow::Result;
use thread_priority::ThreadPriority;

use crate::sync::{const_mutex, Arc, Condvar, Mutex};

use super::processing::{
    processor::{Processor, Progress},
    progresshint::{ProgressHintReceiver, ProgressHintSender, SwitchProgressHintResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Running,
    Suspended,
    Stopping,
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

#[derive(Debug)]
struct Context {
    progress_hint_tx: ProgressHintSender,
}

impl Context {
    pub fn suspend(&self) -> SwitchProgressHintResult {
        self.progress_hint_tx.suspend()
    }

    pub fn resume(&self) -> SwitchProgressHintResult {
        self.progress_hint_tx.resume()
    }

    pub fn terminate(&self) -> SwitchProgressHintResult {
        self.progress_hint_tx.terminate()
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
    context: Context,
    suspender: Arc<Suspender>,
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

#[derive(Debug)]
#[allow(clippy::mutex_atomic)]
struct Suspender {
    suspended: Mutex<bool>,
    condvar: Condvar,
}

impl Suspender {
    pub fn new() -> Self {
        Self {
            suspended: const_mutex(false),
            condvar: Condvar::new(),
        }
    }
}

impl Default for Suspender {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::mutex_atomic)]
impl Suspender {
    fn suspend(&self) -> bool {
        let mut suspended = self.suspended.lock();
        if *suspended {
            // Already suspended
            return false;
        }
        *suspended = true;
        true
    }

    fn resume(&self) -> bool {
        let mut suspended = self.suspended.lock();
        if !*suspended {
            // Not suspended yet
            return false;
        }
        *suspended = false;
        self.condvar.notify_all();
        true
    }

    fn wait_while_suspended(&self) {
        let mut suspended = self.suspended.lock();
        while *suspended {
            self.condvar.wait(&mut suspended);
        }
    }
}

fn thread_fn<N: Notifications, E, P: Processor<E>>(
    progress_hint_rx: &ProgressHintReceiver,
    suspender: &Arc<Suspender>,
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

    log::info!("Running");
    notifications.notify_state_changed(State::Running);

    loop {
        match processor.process(environment, progress_hint_rx)? {
            Progress::Suspended => {
                // The processor might decide to implicitly suspend processing
                // at any time. Therefore we need to explicitly suspend ourselves
                // here, otherwise the thread would not be suspended (see below).
                suspender.suspend();

                log::debug!("Processing suspended");
                notifications.notify_state_changed(State::Suspended);

                suspender.wait_while_suspended();

                log::debug!("Resuming processing");
                notifications.notify_state_changed(State::Running);
            }
            Progress::Terminated => {
                log::debug!("Processing terminated");
                break;
            }
        }
    }

    log::info!("Stopping");
    notifications.notify_state_changed(State::Stopping);

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
    pub fn start(params: Params<E, N, P>) -> Self {
        let progress_hint_rx = ProgressHintReceiver::default();
        let progress_hint_tx = ProgressHintSender::attach(&progress_hint_rx);
        let context = Context { progress_hint_tx };
        let suspender = Arc::new(Suspender::new());
        let join_handle = {
            let suspender = suspender.clone();
            std::thread::spawn({
                move || {
                    adjust_current_thread_priority();
                    // The parameters are mutable within the real-time thread
                    let mut params = params;
                    let result = thread_fn(&progress_hint_rx, &suspender, &mut params);
                    let recovered_params = params;
                    TerminatedThread {
                        result,
                        recovered_params,
                    }
                }
            })
        };
        Self {
            join_handle,
            context,
            suspender,
        }
    }

    pub fn suspend(&self) -> anyhow::Result<bool> {
        // 1st step: Ensure that the thread will block and suspend itself
        // after processing has been suspended
        if !self.suspender.suspend() {
            log::debug!("Already suspending or suspended");
            return Ok(false);
        }
        // 2nd step: Request processing to suspend
        self.context.suspend()?;
        Ok(true)
    }

    pub fn resume(&self) -> anyhow::Result<bool> {
        // 1st step: Ensure that processing either continues or
        // terminates after the thread has been woken up and
        // resumes running
        self.context.resume()?;
        // 2nd step: Wake up the thread
        if !self.suspender.resume() {
            log::debug!("Not suspended yet");
            return Ok(false);
        }
        Ok(true)
    }

    /// Stop the thread
    ///
    /// The thread is stopped after the processor has returned
    /// from the last processing step without interruption.
    pub fn stop(&self) -> anyhow::Result<()> {
        self.abort(|| {})
    }

    /// Stop the thread by aborting processing
    ///
    /// Processing could be interrupted by a side-effect that
    /// intercepts the termination.
    pub fn abort(&self, on_abort: impl FnOnce()) -> anyhow::Result<()> {
        // 1st step: Ensure that processing will terminate
        self.context.terminate()?;
        // 2nd step: Abort processing through a side-effect controlled
        // by the caller. This must intercept the 1st and 3rd step to
        // avoid race conditions!
        on_abort();
        // 3rd step: Finally wake up the thread in case it is suspended.
        // Otherwise it might stay suspended forever.
        self.suspender.resume();
        Ok(())
    }

    pub fn join(self) -> JoinedThread<E, N, P> {
        let Self {
            join_handle,
            context: _,
            suspender: _,
        } = self;
        join_handle
            .join()
            .map(JoinedThread::Terminated)
            .unwrap_or_else(JoinedThread::JoinError)
    }
}
