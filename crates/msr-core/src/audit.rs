//! Structs used for auditing
use std::time::SystemTime;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Activity<S> {
    pub who: S,
    pub when: SystemTime,
}

impl<S> Activity<S> {
    pub fn now(who: impl Into<S>) -> Self {
        Self {
            who: who.into(),
            when: SystemTime::now(),
        }
    }
}
