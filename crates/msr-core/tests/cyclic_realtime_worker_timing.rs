use std::{
    thread,
    time::{Duration, Instant},
};

use msr_core::realtime::worker::{
    progress::{ProgressHint, ProgressHintReceiver},
    thread::{Events, JoinedThread, RecoverableParams, State, TerminatedThread, WorkerThread},
    CompletionStatus, Worker,
};

// Expected upper bound for deviation from nominal cycle timing,
// i.e. range between earliest and latest measured deviation.
const MAX_EXPECTED_JITTER: Duration = Duration::from_millis(1);

#[derive(Default)]
struct CyclicWorkerEnvironment;

#[derive(Default)]
struct CyclicWorkerEvents;

impl Events for CyclicWorkerEvents {
    fn on_state_changed(&mut self, _state: State) {}
}

#[derive(Debug, Clone)]
struct CyclicWorkerParams {
    pub busy_time: Duration,
    pub cycle_time: Duration,
    pub rounds: u32,
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
    pub fn new(params: CyclicWorkerParams) -> Self {
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
        if expected_cycle_start >= actual_cycle_start {
            return expected_cycle_start;
        }
        let elapsed_cycles = actual_cycle_start
            .duration_since(expected_cycle_start)
            .as_secs_f64()
            / self.params.cycle_time.as_secs_f64();
        debug_assert!(elapsed_cycles > 0.0);
        if elapsed_cycles >= 2.0 {
            // We missed at least 1 entire cycle
            let missed_cycles = elapsed_cycles.floor() - 1.0;
            debug_assert!(missed_cycles <= u32::MAX as f64);
            let missed_cycles = missed_cycles.min(u32::MAX as f64) as u32;
            self.measurements.skipped_cycles += missed_cycles;
            // Adjust the deadline of the previous cycle
            let skipped_cycles_duration = missed_cycles * self.params.cycle_time;
            expected_cycle_start + skipped_cycles_duration
        } else {
            expected_cycle_start
        }
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
            while progress_hint_rx.wait_until(cycle_deadline) {
                match progress_hint_rx.peek() {
                    ProgressHint::Continue => {
                        continue;
                    }
                    ProgressHint::Suspend => {
                        return Ok(CompletionStatus::Suspending);
                    }
                    ProgressHint::Finish => {
                        return Ok(CompletionStatus::Finishing);
                    }
                };
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
    let worker_thread = WorkerThread::spawn(recoverable_params);
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
fn cyclic_realtime_worker_timing_run_with_nocapture_to_print_measurements() -> anyhow::Result<()> {
    let params = CyclicWorkerParams {
        busy_time: Duration::from_millis(3),
        cycle_time: Duration::from_millis(5),
        rounds: 1000,
    };
    assert!(params.busy_time <= params.cycle_time);

    let measurements = run_cyclic_worker(params.clone())?;

    assert_eq!(params.rounds, measurements.completed_cycles);
    assert_eq!(0, measurements.skipped_cycles);

    let earliness_avg = measurements.earliness_sum / params.rounds;
    println!(
        "Earliness: max = {} ms, avg = {} ms",
        measurements.earliness_max.as_secs_f64() * 1000.0,
        earliness_avg.as_secs_f64() * 1000.0
    );

    let lateness_avg = measurements.lateness_sum / params.rounds;
    println!(
        "Lateness: max = {} ms, avg = {} ms",
        measurements.lateness_max.as_secs_f64() * 1000.0,
        lateness_avg.as_secs_f64() * 1000.0
    );

    // Upper bound for deviation from nominal cycle timing
    // ||<-        full cycle       ->||<-        full cycle      ->||
    // ||<- ... ->|<- earliness_max ->||<- lateness_max ->|<- ... ->||
    let max_actual_jitter = measurements.earliness_max + measurements.lateness_max;
    assert!(max_actual_jitter <= MAX_EXPECTED_JITTER);

    // As observed on Linux, may vary for other operating systems
    #[cfg(target_os = "linux")]
    assert_eq!(Duration::ZERO, measurements.earliness_max);

    Ok(())
}

#[test]
fn cyclic_realtime_worker_skipped_cycles_run_with_nocapture_to_print_measurements(
) -> anyhow::Result<()> {
    let params = CyclicWorkerParams {
        busy_time: Duration::from_millis(6),
        cycle_time: Duration::from_millis(5),
        rounds: 1000,
    };
    assert!(params.busy_time > params.cycle_time);

    let measurements = run_cyclic_worker(params.clone())?;

    let max_completed_cycles =
        params.rounds as f64 * params.cycle_time.as_secs_f64() / params.busy_time.as_secs_f64();
    assert!(measurements.completed_cycles <= max_completed_cycles.floor() as u32);
    assert!(params.rounds >= measurements.completed_cycles);
    assert_eq!(
        params.rounds - measurements.completed_cycles,
        measurements.skipped_cycles
    );

    let earliness_avg = measurements.earliness_sum / params.rounds;
    println!(
        "Earliness: max = {} ms, avg = {} ms",
        measurements.earliness_max.as_secs_f64() * 1000.0,
        earliness_avg.as_secs_f64() * 1000.0
    );

    let lateness_avg = measurements.lateness_sum / params.rounds;
    println!(
        "Lateness: max = {} ms, avg = {} ms",
        measurements.lateness_max.as_secs_f64() * 1000.0,
        lateness_avg.as_secs_f64() * 1000.0
    );

    // As observed on Linux, may vary for other operating systems
    #[cfg(target_os = "linux")]
    assert_eq!(Duration::ZERO, measurements.earliness_max);

    Ok(())
}
