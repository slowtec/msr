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
    running: AtomicUsize,
    suspending: AtomicUsize,
    finishing: AtomicUsize,
    stopping: AtomicUsize,
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
}

impl Events for SmokeTestEvents {
    fn on_state_changed(&self, state: State) {
        match state {
            State::Unknown => unreachable!(),
            State::Starting => {
                self.state_changed_count
                    .starting
                    .fetch_add(1, Ordering::SeqCst);
            }
            State::Running => {
                self.state_changed_count
                    .running
                    .fetch_add(1, Ordering::SeqCst);
            }
            State::Suspending => {
                self.state_changed_count
                    .suspending
                    .fetch_add(1, Ordering::SeqCst);
                assert_eq!(
                    SwitchProgressHintOk::Accepted {
                        previous_state: ProgressHint::Suspend,
                    },
                    self.progress_hint_tx.resume().expect("resuming")
                );
            }
            State::Finishing => {
                self.state_changed_count
                    .finishing
                    .fetch_add(1, Ordering::SeqCst);
            }
            State::Stopping => {
                self.state_changed_count
                    .stopping
                    .fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

#[test]
fn smoke_test() -> anyhow::Result<()> {
    for expected_perform_work_invocations in 1..10 {
        let worker = SmokeTestWorker::new(expected_perform_work_invocations);
        let progress_hint_rx = ProgressHintReceiver::default();
        let events = SmokeTestEvents::new(ProgressHintSender::attach(&progress_hint_rx));
        let recoverable_params = RecoverableParams {
            progress_hint_rx,
            worker,
            environment: SmokeTestEnvironment,
            events,
        };
        // Real-time thread scheduling might not be supported when running the tests
        // in containers on CI platforms.
        let worker_thread = WorkerThread::spawn(ThreadScheduling::Default, recoverable_params);
        match worker_thread.join() {
            JoinedThread::Terminated(TerminatedThread {
                recovered_params:
                    RecoverableParams {
                        progress_hint_rx: _,
                        worker,
                        environment: _,
                        events,
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
                    events.state_changed_count.starting.load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    events.state_changed_count.stopping.load(Ordering::SeqCst)
                );
                assert_eq!(
                    expected_perform_work_invocations,
                    events.state_changed_count.running.load(Ordering::SeqCst)
                );
                assert_eq!(
                    1,
                    events.state_changed_count.finishing.load(Ordering::SeqCst)
                );
                assert_eq!(
                    events.state_changed_count.running.load(Ordering::SeqCst)
                        - events.state_changed_count.finishing.load(Ordering::SeqCst),
                    events.state_changed_count.suspending.load(Ordering::SeqCst)
                );
            }
            JoinedThread::JoinError(err) => {
                return Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
            }
        }
    }

    Ok(())
}
