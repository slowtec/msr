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
                assert!(
                    self.actual_perform_work_invocations <= self.expected_perform_work_invocations
                );
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

#[test]
fn smoke_test() -> anyhow::Result<()> {
    for expected_perform_work_invocations in 1..10 {
        let worker = SmokeTestWorker::new(expected_perform_work_invocations);
        let progress_hint_rx = ProgressHintReceiver::default();
        let progress_hint_tx = ProgressHintSender::attach(&progress_hint_rx);
        let context = Context {
            progress_hint_rx,
            worker,
            environment: SmokeTestEnvironment,
        };
        // Real-time thread scheduling might not be supported when running the tests
        // in containers on CI platforms.
        let worker_thread = WorkerThread::spawn(context, ThreadScheduling::Default);
        let mut resume_accepted = 0;
        loop {
            match worker_thread.load_state() {
                State::Initial | State::Starting | State::Finishing | State::Running => (),
                State::Suspending => match progress_hint_tx.resume() {
                    Ok(SwitchProgressHintOk::Accepted { .. }) => {
                        resume_accepted += 1;
                    }
                    // The worker thread might already have terminated itself, which in turn
                    // detaches our `ProgressHintSender`.
                    Ok(SwitchProgressHintOk::Ignored) | Err(_) => (),
                },
                State::Terminating => {
                    // Exit loop
                    break;
                }
            }
        }
        match worker_thread.join() {
            JoinedThread::Terminated(TerminatedThread {
                context:
                    Context {
                        progress_hint_rx: _,
                        worker,
                        environment: _,
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
                assert_eq!(expected_perform_work_invocations, resume_accepted + 1,);
            }
            JoinedThread::JoinError(err) => {
                return Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
            }
        }
    }

    Ok(())
}

// Start in suspended state and finish immediately while suspended.
#[test]
fn suspend_before_starting_and_finish_while_suspended() -> anyhow::Result<()> {
    // 0 => perform_work() must never be invoked with ProgressHint::Continue
    let expected_perform_work_invocations = 0;
    let worker = SmokeTestWorker::new(expected_perform_work_invocations);
    let progress_hint_rx = ProgressHintReceiver::default();
    let progress_hint_tx = ProgressHintSender::attach(&progress_hint_rx);
    assert!(matches!(
        progress_hint_tx.suspend(),
        Ok(SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Continue,
        })
    ));
    let context = Context {
        progress_hint_rx,
        worker,
        environment: SmokeTestEnvironment,
    };
    // Real-time thread scheduling might not be supported when running the tests
    // in containers on CI platforms.
    let worker_thread = WorkerThread::spawn(context, ThreadScheduling::Default);
    assert_eq!(State::Suspending, worker_thread.wait_until_not_running());
    assert!(matches!(
        progress_hint_tx.finish(),
        Ok(SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Suspend,
        })
    ));
    match worker_thread.join() {
        JoinedThread::Terminated(TerminatedThread {
            context:
                Context {
                    progress_hint_rx: _,
                    worker,
                    environment: _,
                },
            result,
        }) => {
            result?;
            assert_eq!(1, worker.start_working_invocations);
            assert_eq!(1, worker.finish_working_invocations);
            // Two invocations of perform_work() are expected:
            //  - 1st: ProgressHint::Suspend
            //  - 2nd: ProgressHint::Finish
            assert_eq!(2, worker.actual_perform_work_invocations);
        }
        JoinedThread::JoinError(err) => {
            return Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
        }
    }

    Ok(())
}
