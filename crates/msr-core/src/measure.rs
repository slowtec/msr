use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement<V> {
    /// A time stamp
    pub ts: Instant,

    /// The measured value
    pub val: Option<V>,
}
