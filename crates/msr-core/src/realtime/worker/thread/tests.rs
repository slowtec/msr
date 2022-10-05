use std::sync::atomic::{AtomicUsize, Ordering};

use crate::realtime::worker::progress::{ProgressHint, ProgressHintSender, SwitchProgressHintOk};

use super::*;

struct SmokeTestEnvironment;

#[derive(Default)]
struct SmokeTestWorker {
    start_working_invocations: usize,
    finish_working_invocations: usize,
    actual_perform_work_invocations: usize,
    expected_perform_work_invocations: usize,
}

impl SmokeTestWorker {
    fn new(expected_perform_work_invocations: usize) -> Self {
        Self {
            expected_perform_work_invocations,
            ..Default::default()
        }
    }
}

impl Worker for SmokeTestWorker {
    type Environment = SmokeTestEnvironment;

    fn start_working(&mut self, _env: &mut Self::Environment) -> Result<()> {
        self.start_working_invocations += 1;
        Ok(())
    }

    fn finish_working(&mut self, _env: &mut Self::Environment) -> Result<()> {
        self.finish_working_invocations += 1;
        Ok(())
    }

    fn perform_work(
        &mut self,
        _env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<CompletionStatus> {
        self.actual_perform_work_invocations += 1;
        let progress = match progress_hint_rx.peek() {
            ProgressHint::Continue => {
                if self.actual_perform_work_invocations < self.expected_perform_work_invocations {
                    CompletionStatus::Suspending
                } else {
                    CompletionStatus::Finishing
                }
            }
            ProgressHint::Suspend => CompletionStatus::Suspending,
            ProgressHint::Finish => CompletionStatus::Finishing,
        };
        Ok(progress)
    }
}

#[derive(Default)]
struct StateChangedCount {
    starting: AtomicUsize,
    started: AtomicUsize,
    running: AtomicUsize,
    suspended: AtomicUsize,
    resumed: AtomicUsize,
    finishing: AtomicUsize,
    finished: AtomicUsize,
    terminating: AtomicUsize,
}

struct SmokeTestEvents {
    progress_hint_tx: ProgressHintSender,
    state_changed_count: StateChangedCount,
}

impl SmokeTestEvents {
    fn new(progress_hint_tx: ProgressHintSender) -> Self {
        Self {
            progress_hint_tx,
            state_changed_count: Default::default(),
        }
    }

    fn on_event(&self, event: Event) {
        match event {
            Event::StateChanged(state) => match state {
                State::Unknown => unreachable!(),
                State::Starting => {
                    self.state_changed_count
                        .starting
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Started => {
                    self.state_changed_count
                        .started
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Running => {
                    self.state_changed_count
                        .running
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Suspended => {
                    self.state_changed_count
                        .suspended
                        .fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        SwitchProgressHintOk::Accepted {
                            previous_state: ProgressHint::Suspend,
                        },
                        self.progress_hint_tx.resume().expect("resuming")
                    );
                }
                State::Resumed => {
                    self.state_changed_count
                        .resumed
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Finishing => {
                    self.state_changed_count
                        .finishing
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Finished => {
                    self.state_changed_count
                        .finished
                        .fetch_add(1, Ordering::SeqCst);
                }
                State::Terminating => {
                    self.state_changed_count
                        .terminating
                        .fetch_add(1, Ordering::SeqCst);
                }
            },
        }
    }
}

impl EmitEvent for SmokeTestEvents {
    fn emit_event(&mut self, event: Event) {
        self.on_event(event);
    }
}

#[test]
fn smoke_test() -> anyhow::Result<()> {
    for expected_perform_work_invocations in 1..10 {
        let worker = SmokeTestWorker::new(expected_perform_work_invocations);
        let progress_hint_rx = ProgressHintReceiver::default();
        let event_handler = SmokeTestEvents::new(ProgressHintSender::attach(&progress_hint_rx));
        let context = Context {
            progress_hint_rx,
            worker,
            environment: SmokeTestEnvironment,
            emit_event: event_handler,
        };
        // Real-time thread scheduling might not be supported when running the tests
        // in containers on CI platforms.
        let worker_thread = WorkerThread::spawn(ThreadScheduling::Default, context);
        match worker_thread.join() {
            JoinedThread::Terminated(TerminatedThread {
                context:
                    Context {
                        progress_hint_rx: _,
                        worker,
                        environment: _,
                        emit_event: event_handler,
                    },
                result,
            }) => {
                result?;
                assert_eq!(1, worker.start_working_invocations);
                assert_eq!(1, worker.finish_working_invocations);
                assert_eq!(
                    expected_perform_work_invocations,
                    worker.actual_perform_work_invocations
                );
                assert_eq!(
                    1,
                    event_handler
                        .state_changed_count
                        .starting
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    event_handler
                        .state_changed_count
                        .started
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    expected_perform_work_invocations,
                    event_handler
                        .state_changed_count
                        .running
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    expected_perform_work_invocations - 1,
                    event_handler
                        .state_changed_count
                        .suspended
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    expected_perform_work_invocations - 1,
                    event_handler
                        .state_changed_count
                        .resumed
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    event_handler
                        .state_changed_count
                        .finishing
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    event_handler
                        .state_changed_count
                        .finished
                        .load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    event_handler
                        .state_changed_count
                        .terminating
                        .load(Ordering::SeqCst)
                );
            }
            JoinedThread::JoinError(err) => {
                return Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
            }
        }
    }

    Ok(())
}
