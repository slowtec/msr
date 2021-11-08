use anyhow::Result;

use super::progress_hint::ProgressHintReceiver;

/// Intention after completing a unit of work
///
/// Affects the subsequent control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Working should be suspended
    Suspending,

    /// Working should be finished
    Finishing,
}

/// Callback interface for real-time processing tasks
///
/// All invocations between start_working() and finish_working() will
/// happen on the same thread, including those two clamping functions.
pub trait Worker {
    /// The environment is provided as an external invocation context
    /// to every function of the worker.
    type Environment;

    /// Start working
    ///
    /// Invoked once before the first call to [`Worker::perform_work()`] for
    /// acquiring resources and to perform initialization.
    fn start_working(&mut self, env: &mut Self::Environment) -> Result<()>;

    /// Perform a unit of work
    ///
    /// This function is invoked at least once after [`Worker::start_working()`]
    /// has returned successfully. It will be invoked repeatedly until finally
    /// [`Worker::finish_working()`] is invoked.
    ///
    /// This function is not supposed to mutate the environment.
    fn perform_work(
        &mut self,
        env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<Completion>;

    /// Finish working after terminated
    ///
    /// Invoked once after the last call to [`Worker::perform_work()`] for finalizing
    /// results, releasing resources, or performing cleanup.
    fn finish_working(&mut self, env: &mut Self::Environment) -> Result<()>;
}

/// Wraps a [`Worker`] as a boxed trait object
pub type WorkerBoxed<E> = Box<dyn Worker<Environment = E> + Send + 'static>;

impl<E> Worker for WorkerBoxed<E> {
    type Environment = E;

    fn start_working(&mut self, env: &mut Self::Environment) -> Result<()> {
        (&mut **self).start_working(env)
    }

    fn perform_work(
        &mut self,
        env: &Self::Environment,
        progress_hint_rx: &ProgressHintReceiver,
    ) -> Result<Completion> {
        (&mut **self).perform_work(env, progress_hint_rx)
    }

    fn finish_working(&mut self, env: &mut Self::Environment) -> Result<()> {
        (&mut **self).finish_working(env)
    }
}
