use crate::realtime::progress_hint::{ProgressHint, ProgressHintSender, SwitchProgressHintOk};

use super::*;

struct SmokeTestEnvironment;

#[derive(Default)]
struct SmokeTestWorker {
    start_working_invocations: usize,
    finish_working_invocations: usize,
    actual_do_work_invocations: usize,
    expected_do_work_invocations: usize,
}

impl SmokeTestWorker {
    pub fn new(expected_do_work_invocations: usize) -> Self {
        Self {
            expected_do_work_invocations,
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
    ) -> Result<Completion> {
        self.actual_do_work_invocations += 1;
        let progress = match progress_hint_rx.peek() {
            ProgressHint::Running => {
                if self.actual_do_work_invocations < self.expected_do_work_invocations {
                    Completion::Suspending
                } else {
                    Completion::Terminating
                }
            }
            ProgressHint::Suspending => Completion::Suspending,
            ProgressHint::Terminating => Completion::Terminating,
        };
        Ok(progress)
    }
}

#[derive(Default)]
struct StateChangedCount {
    starting: usize,
    running: usize,
    suspending: usize,
    terminating: usize,
    stopping: usize,
}

struct SmokeTestNotifications {
    progress_hint_tx: ProgressHintSender,
    state_changed_count: StateChangedCount,
}

impl SmokeTestNotifications {
    pub fn new(progress_hint_tx: ProgressHintSender) -> Self {
        Self {
            progress_hint_tx,
            state_changed_count: Default::default(),
        }
    }
}

impl Notifications for SmokeTestNotifications {
    fn notify_state_changed(&mut self, state: State) {
        match state {
            State::Starting => {
                self.state_changed_count.starting += 1;
            }
            State::Running => {
                self.state_changed_count.running += 1;
            }
            State::Suspending => {
                self.state_changed_count.suspending += 1;
                assert_eq!(
                    SwitchProgressHintOk::Accepted {
                        previous_state: ProgressHint::Suspending,
                    },
                    self.progress_hint_tx.resume().expect("resuming")
                );
            }
            State::Terminating => {
                self.state_changed_count.terminating += 1;
            }
            State::Stopping => {
                self.state_changed_count.stopping += 1;
            }
        }
    }
}

#[test]
fn smoke_test() -> anyhow::Result<()> {
    for expected_perform_work_invocations in 1..10 {
        let worker = SmokeTestWorker::new(expected_perform_work_invocations);
        let progress_hint_rx = ProgressHintReceiver::default();
        let notifications =
            SmokeTestNotifications::new(ProgressHintSender::attach(&progress_hint_rx));
        let recoverable_params = RecoverableParams {
            worker,
            environment: SmokeTestEnvironment,
            notifications,
        };
        let worker_thread = WorkerThread::spawn(progress_hint_rx, recoverable_params);
        match worker_thread.join() {
            JoinedThread::Terminated(TerminatedThread {
                recovered_params:
                    RecoverableParams {
                        worker,
                        environment: _,
                        notifications,
                    },
                result,
            }) => {
                result?;
                assert_eq!(1, worker.start_working_invocations);
                assert_eq!(1, worker.finish_working_invocations);
                assert_eq!(
                    expected_perform_work_invocations,
                    worker.actual_do_work_invocations
                );
                assert_eq!(1, notifications.state_changed_count.starting);
                assert_eq!(1, notifications.state_changed_count.stopping);
                assert_eq!(
                    expected_perform_work_invocations,
                    notifications.state_changed_count.running
                );
                assert_eq!(1, notifications.state_changed_count.terminating);
                assert_eq!(
                    notifications.state_changed_count.running
                        - notifications.state_changed_count.terminating,
                    notifications.state_changed_count.suspending
                );
            }
            JoinedThread::JoinError(err) => {
                return Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
            }
        }
    }

    Ok(())
}
