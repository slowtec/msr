use anyhow::Result;

use super::progress_hint::ProgressHintReceiver;

/// Intention after completing a unit of work
///
/// Affects the subsequent control flow outside of the worker's
/// own context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
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

/// Callback interface for real-time processing tasks
///
/// All invocations between start_working_task() and finish_working_task()
/// will happen on the same thread, including those two clamping functions.
///
/// ```puml
/// @startuml
/// participant Worker
///
/// -> Worker: start_working_task()
/// activate Worker
/// <- Worker: started
/////deactivate Worker
///
/// -> Worker: perform_work()
/// activate Worker
/// <- Worker: Completion::Suspending
/// deactivate Worker
///
/// ...
///
/// -> Worker: perform_work()
/////activate Worker
/// <- Worker: Completion::Finishing
/// deactivate Worker
///
/// -> Worker: finish_working_task()
/// activate Worker
/// <- Worker: finished
/// deactivate Worker
/// @enduml
/// ```
pub trait Worker {
    /// The environment is provided as an external invocation context
    /// to every function of the worker.
    type Environment;

    /// Start a new working task
    ///
    /// Invoked once before the first call to [`Worker::perform_work()`] for
    /// acquiring resources and to perform initialization.
    fn start_working_task(&mut self, env: &mut Self::Environment) -> Result<()>;

    /// Perform a unit of work
    ///
    /// Performs work for the current working task until either no more work is
    /// pending or the progress hint indicates that suspending or finishing is
    /// desired.
    ///
    /// This function is invoked at least once after [`Worker::start_working_task()`]
    /// has returned successfully. It will be invoked repeatedly until finally
    /// [`Worker::finish_working_task()`] is invoked.
    ///
    /// This function is not supposed to mutate the environment.
    fn perform_work(
        &mut self,
        env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<Completion>;

    /// Finish the current working task
    ///
    /// Invoked once after the last call to [`Worker::perform_work()`] for finalizing
    /// results, releasing resources, or performing cleanup.
    fn finish_working_task(&mut self, env: &mut Self::Environment) -> Result<()>;
}

/// Wraps a [`Worker`] as a boxed trait object
pub type WorkerBoxed<E> = Box<dyn Worker<Environment = E> + Send + 'static>;

impl<E> Worker for WorkerBoxed<E> {
    type Environment = E;

    fn start_working_task(&mut self, env: &mut Self::Environment) -> Result<()> {
        (&mut **self).start_working_task(env)
    }

    fn perform_work(
        &mut self,
        env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<Completion> {
        (&mut **self).perform_work(env, progress_hint_rx)
    }

    fn finish_working_task(&mut self, env: &mut Self::Environment) -> Result<()> {
        (&mut **self).finish_working_task(env)
    }
}
