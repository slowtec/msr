use std::fmt;

use msr_core::Measurement;

// Re-exports
pub use msr_core::{register::Index, Value, ValueType as Type};

// Generic re-exports of msr-core, specialized with concrete value type
pub type ValueMeasurement = Measurement<Value>;
pub type IndexedValueMeasurement = msr_core::register::IndexedMeasurement<Value>;
pub type ObservedValue = msr_core::register::ObservedValue<Value>;
pub type ObservedValues = msr_core::register::ObservedValues<Value>;

pub type ObservedRegisterValues = msr_core::register::recording::ObservedRegisterValues<Value>;
pub type Record = msr_core::register::recording::Record<Value>;
pub type StoredRecord = msr_core::register::recording::StoredRecord<Value>;

pub type GroupIdValue = String;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GroupId(GroupIdValue);

impl GroupId {
    pub const fn from_value(value: GroupIdValue) -> Self {
        Self(value)
    }

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
