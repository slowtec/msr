use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
};

use anyhow::{anyhow, Result};
use thread_priority::ThreadPriority;

use super::{
    processor::{ProcessControllerBoxed, Processor, ProcessorBoxed},
    Progress, ProgressHint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Running,
    Suspended,
    Stopping,
    Terminating,
}

#[derive(Debug, Clone, Copy)]
pub struct Status {
    pub state: State,
}

pub trait StatusReporter {
    fn report_status(&mut self, status: &Status);
}

pub type StatusReporterBoxed = Box<dyn StatusReporter + Send + 'static>;

const PROGRESS_HINT_RUNNING: u8 = 0;
const PROGRESS_HINT_SUSPENDING: u8 = 1;
const PROGRESS_HINT_TERMINATING: u8 = 2;

#[derive(Debug)]
struct AtomicProgressHint(AtomicU8);

impl AtomicProgressHint {
    pub const fn new() -> Self {
        Self(AtomicU8::new(PROGRESS_HINT_RUNNING))
    }

    pub fn load(&self) -> ProgressHint {
        match self.0.load(Ordering::Acquire) {
            PROGRESS_HINT_RUNNING => ProgressHint::Running,
            PROGRESS_HINT_SUSPENDING => ProgressHint::Suspending,
            PROGRESS_HINT_TERMINATING => ProgressHint::Terminating,
            progress_hint => unreachable!("unexpected progress hint value: {}", progress_hint),
        }
    }

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

    pub fn terminate(&self) {
        self.0.store(PROGRESS_HINT_TERMINATING, Ordering::Release);
    }
}

pub struct Context {
    progress_hint: Arc<AtomicProgressHint>,
    process_controller: ProcessControllerBoxed,
}

impl Context {
    pub fn new(process_controller: ProcessControllerBoxed) -> Self {
        Self {
            progress_hint: Arc::new(AtomicProgressHint::new()),
            process_controller,
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

struct ProcessorEnvironment {
    progress_hint: Arc<AtomicProgressHint>,
}

impl super::processor::Environment for ProcessorEnvironment {
    fn progress_hint(&self) -> ProgressHint {
        self.progress_hint.load()
    }
}

/// Spawn parameters
pub struct Params {
    pub reusable: ReusableParams,
    pub process_controller: ProcessControllerBoxed,
}

/// Reusable spawn parameters
///
/// The parameters are passed into the worker thread when spawned
/// and partially recovered after joining the worker thread for
/// later reuse.
///
/// If joining the work thread fails the parameters will be lost
/// inevitably!
pub struct ReusableParams {
    pub status_reporter: StatusReporterBoxed,
    pub processor: ProcessorBoxed,
}

pub struct Thread {
    context: Context,
    suspender: Arc<Suspender>,
    join_handle: JoinHandle<(ReusableParams, Result<()>)>,
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

fn report_state(status_reporter: &mut dyn StatusReporter, state: State) {
    // TODO: We do not need to store the status, because currently
    // it only contains the control state and nothing else.
    let status = Status { state };
    status_reporter.report_status(&status);
}

fn thread_fn(
    processor_environment: &ProcessorEnvironment,
    suspender: &Arc<Suspender>,
    status_reporter: &mut dyn StatusReporter,
    processor: &mut dyn Processor,
) -> Result<()> {
    log::info!("Starting");
    report_state(status_reporter, State::Starting);

    processor.start_processing()?;

    log::info!("Running");
    report_state(status_reporter, State::Running);

    loop {
        match processor.process(processor_environment) {
            Progress::Suspended => {
                // The processor might decide to implicitly suspend processing
                // at any time. Therefore we need to explicitly suspend ourselves
                // here, otherwise the thread would not be suspended (see below).
                suspender.suspend();

                log::debug!("Processing suspended");
                report_state(status_reporter, State::Suspended);

                suspender.wait_while_suspended();

                log::debug!("Resuming processing");
                report_state(status_reporter, State::Running);
            }
            Progress::Terminated => {
                log::debug!("Processing terminated");
                break;
            }
        }
    }

    log::info!("Stopping");
    report_state(status_reporter, State::Stopping);

    processor.finish_processing()?;

    log::info!("Terminating");
    report_state(status_reporter, State::Terminating);

    Ok(())
}

impl Thread {
    pub fn start(params: Params) -> Self {
        let Params {
            reusable: reusable_params,
            process_controller,
        } = params;
        let context = Context::new(process_controller);
        let suspender = Arc::new(Suspender::default());
        let join_handle = {
            let processor_environment = ProcessorEnvironment {
                progress_hint: context.progress_hint.clone(),
            };
            let suspender = suspender.clone();
            let mut reusable_params = reusable_params;
            std::thread::spawn({
                move || {
                    adjust_current_thread_priority();
                    let ReusableParams {
                        status_reporter,
                        processor,
                    } = &mut reusable_params;
                    let res = thread_fn(
                        &processor_environment,
                        &suspender,
                        &mut **status_reporter,
                        &mut **processor,
                    );
                    (reusable_params, res)
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
            self.context.process_controller.abort_processing();
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
            self.context.process_controller.abort_processing();
        }
        // 3rd step: Wake up the thread in case it is still suspended
        self.suspender.resume();
    }

    pub fn join(self) -> (Option<ReusableParams>, Result<()>) {
        let Self {
            join_handle,
            context: _,
            suspender: _,
        } = self;
        match join_handle.join() {
            Ok((reusable_params, res)) => (Some(reusable_params), res),
            Err(err) => (
                None, // the status sender is lost inevitably if joining failed
                Err(anyhow!("Failed to join thread: {:?}", err)),
            ),
        }
    }
}
