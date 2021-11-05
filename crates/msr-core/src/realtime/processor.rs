use anyhow::Result;

use super::ProgressHintReceiver;

/// Outcome of processing step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Processing has been suspended
    ///
    /// All available work has been finished and processing has been
    /// suspended. The processor might be invoked again after more
    /// work is available
    Suspended,

    /// Processing has terminated
    ///
    /// The processor has terminated and must not be invoked again.
    Terminated,
}

/// Callback interface for real-time processing
pub trait Processor<E> {
    /// Start processing
    ///
    /// Invoked once before the first call to [`Processor::process()`] for
    /// acquiring resources and to perform initialization.
    fn start_processing(&mut self, env: &mut E) -> Result<()>;

    /// Perform the next processing turn
    ///
    /// This function is invoked at least once after [`Processor::start_processing()`]
    /// has returned successfully.
    ///
    /// After returning it is guaranteed to be invoked one more time until finally
    /// `Progress::Terminated` is returned. Then [`Processor::finish_processing()`]
    /// will be invoked.
    ///
    /// This function is not supposed to mutate the environment in contrast
    /// to starting/finishing processing.
    fn process(&mut self, env: &E, progress_hint_rx: &ProgressHintReceiver) -> Result<Progress>;

    /// Finish processing
    ///
    /// Invoked once after the last call to [`Processor::process()`] for updating
    /// the environment, releasing resources, or performing cleanup.
    fn finish_processing(&mut self, env: &mut E) -> Result<()>;
}

/// Wraps a [`Processor`] as a boxed trait object
pub type ProcessorBoxed<E> = Box<dyn Processor<E> + Send + 'static>;

impl<E> Processor<E> for ProcessorBoxed<E> {
    fn start_processing(&mut self, env: &mut E) -> Result<()> {
        (&mut **self).start_processing(env)
    }

    fn finish_processing(&mut self, env: &mut E) -> Result<()> {
        (&mut **self).finish_processing(env)
    }

    fn process(&mut self, env: &E, progress_hint_rx: &ProgressHintReceiver) -> Result<Progress> {
        (&mut **self).process(env, progress_hint_rx)
    }
}
