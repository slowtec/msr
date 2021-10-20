use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

use anyhow::{anyhow, Result};
use thread_priority::ThreadPriority;

use super::{
    processor::{Environment, ProcessingInterceptorBoxed, Processor},
    AtomicProgressHint, Progress,
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

struct Context {
    progress_hint: Arc<AtomicProgressHint>,
    processing_interceptor: ProcessingInterceptorBoxed,
}

impl Context {
    pub fn new(processing_interceptor: ProcessingInterceptorBoxed) -> Self {
        Self {
            progress_hint: Arc::new(AtomicProgressHint::new()),
            processing_interceptor,
        }
    }

    pub fn suspend(&self) -> bool {
        self.progress_hint.suspend()
    }

    pub fn resume(&self) -> bool {
        self.progress_hint.resume()
    }

    pub fn terminate(&self) {
        self.progress_hint.terminate()
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
pub struct Params<E, P, N> {
    pub environment: E,
    pub processor: P,
    pub notifications: N,
    pub processing_interceptor: ProcessingInterceptorBoxed,
}

// Subset of parameters that are passed into the worker thread
// and recovered after joining the thread successfully.
struct ThreadParams<E, P, N> {
    environment: E,
    processor: P,
    notifications: N,
}

#[allow(missing_debug_implementations)]
pub struct Thread<E, P, N> {
    context: Context,
    suspender: Arc<Suspender>,
    join_handle: JoinHandle<(ThreadParams<E, P, N>, Result<()>)>,
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

#[derive(Debug, Default)]
#[allow(clippy::mutex_atomic)]
struct Suspender {
    suspended: Mutex<bool>,
    condvar: Condvar,
}

#[allow(clippy::mutex_atomic)]
impl Suspender {
    fn suspend(&self) -> bool {
        let mut suspended = self
            .suspended
            .lock()
            .expect("lock suspended mutex to suspend");
        if *suspended {
            // Already suspended
            return false;
        }
        *suspended = true;
        true
    }

    fn resume(&self) -> bool {
        let mut suspended = self
            .suspended
            .lock()
            .expect("lock suspended mutex to resume");
        if !*suspended {
            // Not suspended yet
            return false;
        }
        *suspended = false;
        self.condvar.notify_all();
        true
    }

    fn wait_while_suspended(&self) {
        let mut suspended = self.suspended.lock().expect("lock suspended mutex");
        while *suspended {
            suspended = self.condvar.wait(suspended).expect("wait while suspended");
        }
    }
}

fn thread_fn<E: Environment, P: Processor<E>, N: Notifications>(
    progress_hint: Arc<AtomicProgressHint>,
    suspender: &Arc<Suspender>,
    environment: &mut E,
    processor: &mut P,
    notifications: &mut N,
) -> Result<()> {
    log::info!("Starting");
    notifications.notify_state_changed(State::Starting);

    processor.start_processing(environment, progress_hint)?;

    log::info!("Running");
    notifications.notify_state_changed(State::Running);

    loop {
        match processor.process(environment) {
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

impl<E, P, N> Thread<E, P, N>
where
    E: Environment + Send + 'static,
    P: Processor<E> + Send + 'static,
    N: Notifications + Send + 'static,
{
    pub fn start(params: Params<E, P, N>) -> Self {
        let Params {
            mut environment,
            mut notifications,
            mut processor,
            processing_interceptor,
        } = params;
        let context = Context::new(processing_interceptor);
        let suspender = Arc::new(Suspender::default());
        let join_handle = {
            let progress_hint = context.progress_hint.clone();
            let suspender = suspender.clone();
            std::thread::spawn({
                move || {
                    adjust_current_thread_priority();
                    let res = thread_fn(
                        progress_hint,
                        &suspender,
                        &mut environment,
                        &mut processor,
                        &mut notifications,
                    );
                    let thread_params = ThreadParams {
                        environment,
                        notifications,
                        processor,
                    };
                    (thread_params, res)
                }
            })
        };
        Self {
            join_handle,
            context,
            suspender,
        }
    }

    pub fn suspend(&self, abort_processing: bool) -> bool {
        // 1st step: Ensure that the thread will block and suspend itself
        // after processing has been suspended
        if !self.suspender.suspend() {
            log::debug!("Already suspending or suspended");
            return false;
        }
        // 2nd step: Request processing to suspend
        self.context.suspend();
        // 3rd step: Abort any processing if requested
        if abort_processing {
            self.context.processing_interceptor.abort_processing();
        }
        true
    }

    pub fn resume(&self) -> bool {
        // 1st step: Ensure that processing either continues or
        // terminates after the thread has been woken up and
        // resumes running
        self.context.resume();
        // 2nd step: Wake up the thread
        if !self.suspender.resume() {
            log::debug!("Not suspended yet");
            return false;
        }
        true
    }

    pub fn stop(&self, abort_processing: bool) {
        // 1st step: Ensure that processing will terminate
        self.context.terminate();
        // 2nd step: Abort any processing if requested
        if abort_processing {
            self.context.processing_interceptor.abort_processing();
        }
        // 3rd step: Wake up the thread in case it is still suspended
        self.suspender.resume();
    }

    pub fn join(self) -> (Option<Params<E, P, N>>, Result<()>) {
        let Self {
            join_handle,
            context,
            suspender: _,
        } = self;
        let Context {
            processing_interceptor,
            progress_hint: _,
        } = context;
        match join_handle.join() {
            Ok((thread_params, res)) => {
                let ThreadParams {
                    environment,
                    notifications,
                    processor,
                } = thread_params;
                let params = Params {
                    environment,
                    notifications,
                    processor,
                    processing_interceptor,
                };
                (Some(params), res)
            }
            Err(err) => (
                None, // the parameters are lost inevitably if joining failed
                Err(anyhow!("Failed to join thread: {:?}", err)),
            ),
        }
    }
}
