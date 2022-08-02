use std::{
    fmt,
    time::{Duration, Instant},
};

use msr_core::{
    realtime::worker::{
        progress::{ProgressHint, ProgressHintReceiver},
        thread::{
            Events, JoinedThread, RecoverableParams, State, TerminatedThread, ThreadScheduling,
            WorkerThread,
        },
        CompletionStatus, Worker,
    },
    thread,
};

#[derive(Default)]
struct CyclicWorkerEnvironment;

#[derive(Default)]
struct CyclicWorkerEvents;

impl Events for CyclicWorkerEvents {
    fn on_state_changed(&self, _state: State) {}
}

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
    rounds: u32,
    timing: CyclicWorkerTiming,
}

#[derive(Debug, Default)]
struct CyclicWorkerMeasurements {
    completed_cycles: u32,
    skipped_cycles: u32,
    earliness_sum: Duration,
    earliness_max: Duration,
    lateness_sum: Duration,
    lateness_max: Duration,
}

struct CyclicWorker {
    params: CyclicWorkerParams,
    measurements: CyclicWorkerMeasurements,
}

impl CyclicWorker {
    fn new(params: CyclicWorkerParams) -> Self {
        Self {
            params,
            measurements: Default::default(),
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
            self.measurements.skipped_cycles += missed_cycles;
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
        while self.measurements.completed_cycles + self.measurements.skipped_cycles
            < self.params.rounds
        {
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

            self.measurements.completed_cycles += 1;
        }
        Ok(CompletionStatus::Finishing)
    }
}

fn run_cyclic_worker(params: CyclicWorkerParams) -> anyhow::Result<CyclicWorkerMeasurements> {
    let worker = CyclicWorker::new(params);
    let progress_hint_rx = ProgressHintReceiver::default();
    let recoverable_params = RecoverableParams {
        progress_hint_rx,
        worker,
        environment: CyclicWorkerEnvironment,
        events: CyclicWorkerEvents,
    };
    let worker_thread =
        WorkerThread::spawn(ThreadScheduling::RealtimeOrDefault, recoverable_params);
    match worker_thread.join() {
        JoinedThread::Terminated(TerminatedThread {
            recovered_params:
                RecoverableParams {
                    progress_hint_rx: _,
                    worker,
                    environment: _,
                    events: _,
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

#[test]
// This test often fails on GitHub CI due to timing issues
// and is only supposed to be executed locally.
#[ignore]
fn cyclic_realtime_worker_timing() -> anyhow::Result<()> {
    for timing in [CyclicWorkerTiming::Sleeping, CyclicWorkerTiming::Waiting] {
        let params = CyclicWorkerParams {
            busy_time: Duration::from_millis(3),
            cycle_time: Duration::from_millis(5),
            rounds: 1000,
            timing,
        };
        assert!(params.busy_time <= params.cycle_time);

        let measurements = run_cyclic_worker(params.clone())?;

        let lateness_avg = measurements.lateness_sum / params.rounds;
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

        assert_eq!(params.rounds, measurements.completed_cycles);
        assert_eq!(0, measurements.skipped_cycles);
    }

    Ok(())
}

#[test]
fn cyclic_realtime_worker_skipped_cycles() -> anyhow::Result<()> {
    for timing in [CyclicWorkerTiming::Sleeping, CyclicWorkerTiming::Waiting] {
        let params = CyclicWorkerParams {
            busy_time: Duration::from_millis(7),
            cycle_time: Duration::from_millis(5),
            rounds: 1000,
            timing,
        };
        assert!(params.busy_time > params.cycle_time);

        let measurements = run_cyclic_worker(params.clone())?;

        assert_eq!(Duration::ZERO, measurements.earliness_max);
        assert_eq!(Duration::ZERO, measurements.earliness_sum);

        // We are expecting to miss at least 1 cycle
        assert!(measurements.lateness_max > params.cycle_time);
        assert!(measurements.lateness_max >= params.busy_time);
        assert!(params.rounds > measurements.completed_cycles);

        // The number of actual cycles might be higher than the number of rounds
        // if we miss cycles during the last round!
        assert!(params.rounds <= measurements.completed_cycles + measurements.skipped_cycles);

        let max_completed_cycles =
            params.rounds as f64 * params.cycle_time.as_secs_f64() / params.busy_time.as_secs_f64();
        assert!(measurements.completed_cycles <= max_completed_cycles.floor() as u32);
    }

    Ok(())
}
