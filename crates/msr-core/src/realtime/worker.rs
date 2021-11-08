use anyhow::Result;

use super::progress_hint::ProgressHintReceiver;

/// Intention after completing a chunk of work
///
/// Affects the control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Working should be suspended
    Suspending,

    /// Working should be terminated
    Terminating,
}

/// Callback interface for real-time processing
pub trait Worker {
    type Environment;

    /// Start working
    ///
    /// Invoked once before the first call to [`Worker::perform_work()`] for
    /// acquiring resources and to perform initialization.
    fn start_working(&mut self, env: &mut Self::Environment) -> Result<()>;

    /// Perform the next chunk of work
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
