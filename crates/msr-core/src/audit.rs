//! Structs used for auditing
use crate::time::Timestamp;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Activity<T> {
    pub when: Timestamp,
    pub who: T,
}

impl<T> Activity<T> {
    pub fn now(who: impl Into<T>) -> Self {
        Self {
            when: Timestamp::now(),
            who: who.into(),
        }
    }
}
