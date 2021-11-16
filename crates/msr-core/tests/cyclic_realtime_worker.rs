use std::{
    thread,
    time::{Duration, Instant},
};

use msr_core::realtime::worker::{
    progress::{ProgressHint, ProgressHintReceiver},
    thread::{Events, JoinedThread, RecoverableParams, State, TerminatedThread, WorkerThread},
    CompletionStatus, Worker,
};

const BUSY_TIME: Duration = Duration::from_millis(3);

const IDLE_TIME: Duration = Duration::from_millis(2);

const ROUNDS: u32 = 1000;

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
    pub idle_time: Duration,
    pub rounds: u32,
}

impl CyclicWorkerParams {
    pub fn cycle_time(&self) -> Duration {
        self.busy_time + self.idle_time
    }
}

#[derive(Debug, Default)]
struct CyclicWorkerMeasurements {
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
        let mut previous_cycle_deadline = None;
        for _ in 0..=self.params.rounds {
            let actual_cycle_start = Instant::now();
            let expected_cycle_start =
                if let Some(previous_cycle_deadline) = previous_cycle_deadline {
                    if actual_cycle_start < previous_cycle_deadline {
                        let earliness = previous_cycle_deadline.duration_since(actual_cycle_start);
                        self.measurements.earliness_sum += earliness;
                        if self.measurements.earliness_max < earliness {
                            self.measurements.earliness_max = earliness;
                        }
                    } else {
                        let lateness = actual_cycle_start.duration_since(previous_cycle_deadline);
                        self.measurements.lateness_sum += lateness;
                        if self.measurements.lateness_max < lateness {
                            self.measurements.lateness_max = lateness;
                        }
                    }
                    previous_cycle_deadline
                } else {
                    actual_cycle_start
                };
            let cycle_deadline = expected_cycle_start + self.params.cycle_time();
            match progress_hint_rx.peek() {
                ProgressHint::Continue => (),
                ProgressHint::Suspend | ProgressHint::Finish => {
                    panic!("benchmark interrupted");
                }
            };
            // Busy
            thread::sleep(self.params.busy_time);
            // Idle
            if progress_hint_rx.wait_until(cycle_deadline) {
                panic!("benchmark interrupted");
            }
            previous_cycle_deadline = Some(cycle_deadline);
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
fn cyclic_realtime_worker_latencies_run_with_nocapture_to_print_measurements() -> anyhow::Result<()>
{
    let params = CyclicWorkerParams {
        busy_time: BUSY_TIME,
        idle_time: IDLE_TIME,
        rounds: ROUNDS,
    };

    let measurements = run_cyclic_worker(params)?;

    let earliness_avg = measurements.earliness_sum / ROUNDS;
    println!(
        "Earliness: max = {} ms, avg = {} ms",
        measurements.earliness_max.as_secs_f64() * 1000.0,
        earliness_avg.as_secs_f64() * 1000.0
    );

    let lateness_avg = measurements.lateness_sum / ROUNDS;
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
