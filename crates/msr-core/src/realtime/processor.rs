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
    /// Invoked once before the first call to `Processor::process()` to
    /// acquire resources and to perform initialization.
    ///
    /// Might be invoked again after processing has been finished successfully.
    fn start_processing(
        &mut self,
        env: &mut E,
        progress_hint_rx: ProgressHintReceiver,
    ) -> Result<()>;

    /// Finish processing
    ///
    /// Invoked once after the last call to `Processor::process()` to
    /// perform cleanup and to release resources.
    ///
    /// Might be invoked again after processing has been restarted successfully.
    fn finish_processing(&mut self, env: &mut E) -> Result<()>;

    /// Start or resume processing
    ///
    /// The first invocation starts the processing. On return execution
    /// has either been suspended or terminated.
    ///
    /// After processing has been suspended it might be resumed by invoking
    /// this function repeatedly until eventually `Progress::Terminated`
    /// is returned.
    ///
    /// Returning `Progress::Suspended` ensures that this function is invoked
    /// at least once again, allowing the processor to finish any pending
    /// tasks before finally terminating.
    ///
    /// This function is not supposed to mutate the environment in contrast
    /// to starting/finishing processing.
    fn process(&mut self, env: &E) -> Result<Progress>;
}

/// Wraps a [`Processor`] as a boxed trait object
pub type ProcessorBoxed<E> = Box<dyn Processor<E> + Send + 'static>;

impl<E> Processor<E> for ProcessorBoxed<E> {
    fn start_processing(
        &mut self,
        env: &mut E,
        progress_hint_rx: ProgressHintReceiver,
    ) -> Result<()> {
        (&mut **self).start_processing(env, progress_hint_rx)
    }

    fn finish_processing(&mut self, env: &mut E) -> Result<()> {
        (&mut **self).finish_processing(env)
    }

    fn process(&mut self, env: &E) -> Result<Progress> {
        (&mut **self).process(env)
    }
}
