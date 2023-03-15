use anyhow::Result;

pub mod progress;
use self::progress::ProgressHintReceiver;

pub mod thread;

/// Completion status
///
/// Reflects the intention on how to proceed after performing some
/// work.
///
/// Supposed to affect the subsequent control flow outside of the
/// worker's context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStatus {
    /// Working should be suspended
    ///
    /// The worker currently has no more pending work to do.
    Suspending,

    /// Working should be finished
    ///
    /// The worker has accomplished its task and expects to be
    /// finished.
    Finishing,
}

/// Callback interface for performing work under real-time constraints
///
/// All invocations between `start_working`() and `finish_working`()
/// will happen on the same thread, including those two clamping functions.
///
/// ```puml
/// @startuml
/// participant Worker
///
/// -> Worker: start_working()
/// activate Worker
/// <- Worker: started
/// deactivate Worker
///
/// -> Worker: perform_work()
/// activate Worker
/// <- Worker: CompletionStatus::Suspending
/// deactivate Worker
///
/// ...
///
/// -> Worker: perform_work()
/// activate Worker
/// <- Worker: CompletionStatus::Finishing
/// deactivate Worker
///
/// -> Worker: finish_working()
/// activate Worker
/// <- Worker: finished
/// deactivate Worker
/// @enduml
/// ```
pub trait Worker {
    /// The environment is provided as an external invocation context
    /// to every function of the worker.
    type Environment;

    /// Start working
    ///
    /// Invoked once before the first call to [`Worker::perform_work()`] for
    /// acquiring resources and initializing the internal state.
    fn start_working(&mut self, env: &mut Self::Environment) -> Result<()>;

    /// Perform work
    ///
    /// Make progress until work is either interrupted by a progress hint
    /// or done.
    ///
    /// This function is invoked at least once after [`Worker::start_working()`]
    /// has returned successfully. It will be invoked repeatedly until finally
    /// [`Worker::finish_working()`] is invoked.
    ///
    /// This function is not supposed to mutate the environment.
    ///
    /// Returns a completion status that indicates how to proceed.
    fn perform_work(
        &mut self,
        env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<CompletionStatus>;

    /// Finish working
    ///
    /// Invoked once after the last call to [`Worker::perform_work()`] for
    /// finalizing results, releasing resources, and performing cleanup.
    fn finish_working(&mut self, env: &mut Self::Environment) -> Result<()>;
}
