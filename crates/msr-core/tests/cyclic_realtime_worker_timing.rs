use std::{
    fmt,
    sync::mpsc,
    time::{Duration, Instant},
};

use msr_core::{
    realtime::worker::{
        progress::{ProgressHint, ProgressHintReceiver, ProgressHintSender},
        thread::{Context, JoinedThread, State, TerminatedThread, ThreadScheduling, WorkerThread},
        CompletionStatus, Worker,
    },
    thread,
};

#[derive(Default)]
struct CyclicWorkerEnvironment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CyclicWorkerTiming {
    /// Avoid being blocked by lower priority threads due to priority
    /// inversion
    ///
    /// This strategy should be used for short, periodic control loops
    /// with real-time requirements.
    ///
    /// The responsiveness regarding progress hint updates depends on
    /// the cycle time as progress hint updates will not be checked
    /// until the start of the next cycle.
    Sleeping,

    /// Interrupt sleeping when progress hint updates arrive
    Waiting,
}

// Expected upper bound for deviation from nominal cycle timing,
// i.e. range between earliest and latest measured deviation.
//
// The different limits are required for the tests to finish successfully
// on GitHub CI where real-time thread scheduling is not supported.
const fn max_expected_jitter(timing: CyclicWorkerTiming) -> Duration {
    match timing {
        CyclicWorkerTiming::Sleeping => Duration::from_millis(1),
        CyclicWorkerTiming::Waiting => Duration::from_millis(3),
    }
}

impl fmt::Display for CyclicWorkerTiming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sleeping => f.write_str("sleeping"),
            Self::Waiting => f.write_str("waiting"),
        }
    }
}

#[derive(Debug, Clone)]
struct CyclicWorkerParams {
    busy_time: Duration,
    cycle_time: Duration,
    timing: CyclicWorkerTiming,
}

#[derive(Debug, Clone, Default)]
struct CyclicWorkerMeasurements {
    cycles_completed_count: u32,
    cycles_skipped: u32,
    earliness_sum: Duration,
    earliness_max: Duration,
    lateness_sum: Duration,
    lateness_max: Duration,
}

struct CyclicWorker {
    params: CyclicWorkerParams,
    measurements: CyclicWorkerMeasurements,
    measurements_tx: mpsc::Sender<CyclicWorkerMeasurements>,
}

impl CyclicWorker {
    fn new(
        params: CyclicWorkerParams,
        measurements_tx: mpsc::Sender<CyclicWorkerMeasurements>,
    ) -> Self {
        Self {
            params,
            measurements: Default::default(),
            measurements_tx,
        }
    }

    fn update_timing_measurements(
        &mut self,
        expected_cycle_start: Instant,
        actual_cycle_start: Instant,
    ) {
        // Measure deviation/jitter of expected vs. actual timing
        if actual_cycle_start < expected_cycle_start {
            let earliness = expected_cycle_start.duration_since(actual_cycle_start);
            self.measurements.earliness_sum += earliness;
            if self.measurements.earliness_max < earliness {
                self.measurements.earliness_max = earliness;
            }
        } else {
            let lateness = actual_cycle_start.duration_since(expected_cycle_start);
            self.measurements.lateness_sum += lateness;
            if self.measurements.lateness_max < lateness {
                self.measurements.lateness_max = lateness;
            }
        }
    }

    fn skip_missed_cycles(
        &mut self,
        expected_cycle_start: Instant,
        actual_cycle_start: Instant,
    ) -> Instant {
        msr_core::control::cyclic::skip_missed_cycles(
            self.params.cycle_time,
            expected_cycle_start,
            actual_cycle_start,
        )
        .unwrap_or_else(|(expected_cycle_start, missed_cycles)| {
            self.measurements.cycles_skipped += missed_cycles;
            expected_cycle_start
        })
    }
}

impl Worker for CyclicWorker {
    type Environment = CyclicWorkerEnvironment;

    fn start_working(&mut self, _env: &mut Self::Environment) -> anyhow::Result<()> {
        Ok(())
    }

    fn finish_working(&mut self, _env: &mut Self::Environment) -> anyhow::Result<()> {
        Ok(())
    }

    fn perform_work(
        &mut self,
        _env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> anyhow::Result<CompletionStatus> {
        let mut cycle_deadline = Instant::now();
        loop {
            // Idle: Wait for the current cycle to end
            match self.params.timing {
                CyclicWorkerTiming::Sleeping => {
                    // Depending on the use case and cycle time
                    // the progress hint might also be checked
                    // before sleeping
                    thread::sleep_until(cycle_deadline);
                    // Check for a new progress hint only once
                    // before starting the next cycle
                    match progress_hint_rx.peek() {
                        ProgressHint::Continue => (),
                        ProgressHint::Suspend => {
                            return Ok(CompletionStatus::Suspending);
                        }
                        ProgressHint::Finish => {
                            return Ok(CompletionStatus::Finishing);
                        }
                    };
                }
                CyclicWorkerTiming::Waiting => {
                    while progress_hint_rx.wait_until(cycle_deadline) {
                        match progress_hint_rx.peek() {
                            ProgressHint::Continue => (),
                            ProgressHint::Suspend => {
                                return Ok(CompletionStatus::Suspending);
                            }
                            ProgressHint::Finish => {
                                return Ok(CompletionStatus::Finishing);
                            }
                        };
                    }
                }
            }

            // Start the next cycle
            let cycle_start = Instant::now();
            debug_assert!(cycle_start >= cycle_deadline);
            self.update_timing_measurements(cycle_deadline, cycle_start);
            cycle_deadline = self.skip_missed_cycles(cycle_deadline, cycle_start);
            cycle_deadline += self.params.cycle_time;

            // Busy: Perform work (simulated by sleeping)
            thread::sleep(self.params.busy_time);

            self.measurements.cycles_completed_count += 1;
            if self
                .measurements_tx
                .send(self.measurements.clone())
                .is_err()
            {
                // Abort if all receivers disappeared. This should never happen during tests.
                unreachable!("Failed to submit results from worker thread");
                //return Ok(CompletionStatus::Finishing);
            }
        }
    }
}

fn run_cyclic_worker(params: CyclicWorkerParams) -> anyhow::Result<CyclicWorkerMeasurements> {
    let (measurements_tx, measurements_rx) = mpsc::channel();
    let cycle_time = params.cycle_time;
    let worker = CyclicWorker::new(params, measurements_tx);
    let progress_hint_rx = ProgressHintReceiver::default();
    let progress_hint_tx = ProgressHintSender::attach(&progress_hint_rx);
    let context = Context {
        progress_hint_rx,
        worker,
        environment: CyclicWorkerEnvironment,
    };
    let worker_thread = WorkerThread::spawn(context, ThreadScheduling::RealtimeOrDefault);
    let mut suspended_count = 0;
    let mut resumed_count = 0;
    let mut cycles_completed_count = 0;
    let mut finished = false;
    let mut exit_loop = false;
    while !exit_loop {
        match worker_thread.load_state() {
            State::Initial | State::Starting | State::Running | State::Finishing => {
                // These (intermediate) states might not be visible when reading
                // the last state at arbitrary times from an atomic and cannot
                // be used for controlling the control flow of the test!
            }
            State::Suspending => {
                assert!(resumed_count <= suspended_count);
                if resumed_count < suspended_count {
                    progress_hint_tx.resume().expect("resumed");
                    resumed_count += 1;
                }
            }
            State::Terminating => {
                exit_loop = true;
                // Drain the channel one last time after the worker thread has
                // exited its process_work() function. This is required to not
                // discard any results!
            }
        }
        // Keep draining the channel until no more results are received.
        // The timeout is tunable, here we use twice the expected cycle time.
        // If the timeout is shorter the outer loop will repeat more often
        // and the latency for detecting state changed is reduced at the
        // cost of consuming more CPU cycles.
        loop {
            match measurements_rx.recv_timeout(2 * cycle_time) {
                Ok(measurements) => {
                    assert_eq!(
                        cycles_completed_count + 1,
                        measurements.cycles_completed_count
                    );
                    cycles_completed_count = measurements.cycles_completed_count;
                    if !finished && suspended_count == resumed_count {
                        if cycles_completed_count >= CYCLES {
                            // Request the worker to finish asap.
                            progress_hint_tx.finish().expect("finished");
                            finished = true;
                            // Continue receiving measurements until the worker
                            // the worker has finished.
                        } else if cycles_completed_count % 100 == 0 {
                            // Suspend/resume the worker periodically
                            progress_hint_tx.suspend().expect("suspended");
                            assert_eq!(State::Suspending, worker_thread.wait_until_not_running());
                            suspended_count += 1;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Start over, i.e. exit the inner loop and continue in the outer loop
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    unreachable!();
                }
            }
        }
    }
    assert_eq!(suspended_count, resumed_count);
    assert!(finished);
    match worker_thread.join() {
        JoinedThread::Terminated(TerminatedThread {
            context:
                Context {
                    progress_hint_rx: _,
                    worker,
                    environment: _,
                },
            result,
        }) => result
            .map(|()| worker.measurements)
            .map_err(|err| anyhow::anyhow!("Worker thread terminated with error: {:?}", err)),
        JoinedThread::JoinError(err) => {
            Err(anyhow::anyhow!("Failed to join worker thread: {:?}", err))
        }
    }
}

const CYCLES: u32 = 1000;

#[test]
// This test often fails on GitHub CI due to timing issues
// and is only supposed to be executed locally.
#[ignore]
fn cyclic_realtime_worker_timing_no_cycles_skipped() -> anyhow::Result<()> {
    for timing in [CyclicWorkerTiming::Sleeping, CyclicWorkerTiming::Waiting] {
        let params = CyclicWorkerParams {
            busy_time: Duration::from_millis(3),
            cycle_time: Duration::from_millis(5),
            timing,
        };
        assert!(params.busy_time <= params.cycle_time);

        let measurements = run_cyclic_worker(params.clone())?;
        assert!(measurements.cycles_completed_count >= CYCLES);
        assert_eq!(measurements.cycles_skipped, 0);

        let lateness_avg = measurements.lateness_sum / CYCLES;
        println!(
            "Lateness ({}): max = {:.3} ms, avg = {:.3} ms",
            params.timing,
            measurements.lateness_max.as_secs_f64() * 1000.0,
            lateness_avg.as_secs_f64() * 1000.0
        );

        assert_eq!(Duration::ZERO, measurements.earliness_max);
        assert_eq!(Duration::ZERO, measurements.earliness_sum);

        // Upper bound for deviation from nominal cycle timing
        // ||<-        full cycle       ->||<-        full cycle      ->||
        // ||<- ... ->|<- earliness_max ->||<- lateness_max ->|<- ... ->||
        let max_actual_jitter = measurements.earliness_max + measurements.lateness_max;
        assert!(max_actual_jitter <= max_expected_jitter(params.timing));
    }

    Ok(())
}

#[test]
fn cyclic_realtime_worker_timing_with_cycles_skipped() -> anyhow::Result<()> {
    for timing in [CyclicWorkerTiming::Sleeping, CyclicWorkerTiming::Waiting] {
        let params = CyclicWorkerParams {
            busy_time: Duration::from_millis(7),
            cycle_time: Duration::from_millis(5),
            timing,
        };
        // Check precondition for this test that ensures to skip some cycles.
        assert!(params.busy_time > params.cycle_time);

        let measurements = run_cyclic_worker(params.clone())?;
        assert!(measurements.cycles_completed_count >= CYCLES);
        // We are expecting to miss at least 1 cycle
        assert!(measurements.cycles_skipped > 0);

        assert_eq!(Duration::ZERO, measurements.earliness_max);
        assert_eq!(Duration::ZERO, measurements.earliness_sum);

        // The maximum lateness must exceed the cycle time after missing
        // at least 1 cycle.
        assert!(measurements.lateness_max > params.cycle_time);
        // And it must also be greater or equal than the busy_time, which
        // exceeds the cycle_time for this tests.
        assert!(measurements.lateness_max >= params.busy_time);
    }

    Ok(())
}
