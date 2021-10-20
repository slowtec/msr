use std::sync::Arc;

use anyhow::Result;

use super::{AtomicProgressHint, Progress, ProgressHint};

pub trait Environment {
    /// Indicates how to make progress
    ///
    /// The progress hint should be checked every now and then. In
    /// particular before/after long running operations during
    /// processing.
    fn progress_hint(&self) -> ProgressHint;
}

pub trait Processor<E: Environment> {
    /// Start processing
    ///
    /// Invoked once before the first call to `Processor::process()` to
    /// acquire resources and to perform initialization.
    ///
    /// Might be invoked again after processing has been finished successfully.
    fn start_processing(
        &mut self,
        env: &mut E,
        progress_hint: Arc<AtomicProgressHint>,
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
    fn process(&mut self, env: &E) -> Progress;
}

pub type ProcessorBoxed<E> = Box<dyn Processor<E> + Send + 'static>;

impl<E> Processor<E> for ProcessorBoxed<E>
where
    E: Environment,
{
    fn start_processing(
        &mut self,
        env: &mut E,
        progress_hint: Arc<AtomicProgressHint>,
    ) -> Result<()> {
        (&mut **self).start_processing(env, progress_hint)
    }

    fn finish_processing(&mut self, env: &mut E) -> Result<()> {
        (&mut **self).finish_processing(env)
    }

    fn process(&mut self, env: &E) -> Progress {
        (&mut **self).process(env)
    }
}

pub trait ProcessingInterceptor {
    /// Request to abort processing asap
    ///
    /// Requests the processor to return from `Processor::process()` early
    /// without finishing the pending work.
    fn abort_processing(&self);
}

pub type ProcessingInterceptorBoxed = Box<dyn ProcessingInterceptor + Send + 'static>;
