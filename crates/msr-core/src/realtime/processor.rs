use anyhow::Result;

use super::{Progress, ProgressHint};

pub trait Environment {
    /// Indicates how to make progress
    ///
    /// The progress hint should be checked every now and then. In
    /// particular before/after long running operations during
    /// processing.
    fn progress_hint(&self) -> ProgressHint;
}

pub trait Processor {
    /// Start processing
    ///
    /// Invoked once before the first call to `Processor::process()` to
    /// acquire resources and to perform initialization.
    ///
    /// Might be invoked again after processing has been finished successfully.
    fn start_processing(&mut self) -> Result<()>;

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
    fn process(&mut self, env: &dyn Environment) -> Progress;

    /// Finish processing
    ///
    /// Invoked once after the last call to `Processor::process()` to
    /// perform cleanup and to release resources.
    ///
    /// Might be invoked again after processing has been restarted successfully.
    fn finish_processing(&mut self) -> Result<()>;
}

pub type ProcessorBoxed = Box<dyn Processor + Send + 'static>;

pub trait ProcessingInterceptor {
    /// Request to abort processing asap
    ///
    /// Requests the processor to return from `Processor::process()` early
    /// without finishing the pending work.
    fn abort_processing(&self);
}

pub type ProcessingInterceptorBoxed = Box<dyn ProcessingInterceptor + Send + 'static>;
