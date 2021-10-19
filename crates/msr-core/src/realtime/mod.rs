pub mod processor;
pub mod worker_thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Suspended,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressHint {
    /// Processing should continue
    Running,

    /// Processing should be suspended
    Suspending,

    /// Processing should be terminated
    Terminating,
}
