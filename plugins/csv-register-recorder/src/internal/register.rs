use std::fmt;

// Re-exports
pub use msr_core::{register::Index, Value, ValueType as Type};

pub type ObservedRegisterValues = msr_core::register::recorder::ObservedRegisterValues<Value>;
pub type Record = msr_core::register::recorder::Record<Value>;
pub type StoredRecord = msr_core::register::recorder::StoredRecord<Value>;

pub type GroupIdValue = String;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GroupId(GroupIdValue);

impl GroupId {
    #[must_use]
    pub const fn from_value(value: GroupIdValue) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_value(self) -> GroupIdValue {
        let GroupId(value) = self;
        value
    }
}

impl From<GroupIdValue> for GroupId {
    fn from(from: GroupIdValue) -> Self {
        Self::from_value(from)
    }
}

impl From<GroupId> for GroupIdValue {
    fn from(from: GroupId) -> Self {
        from.into_value()
    }
}

impl AsRef<str> for GroupId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}
