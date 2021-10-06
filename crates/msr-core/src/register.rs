use std::fmt;

use crate::{time::SystemTimeInstant, Measurement};

pub use crate::{Value, ValueType as Type};

/// Address of a register
///
/// Each register is addressed by a uniform, 64-bit unsigned integer value.
pub type IndexValue = u64;

/// Newtype for addressing a single register
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Index(IndexValue);

impl Index {
    pub const fn new(value: IndexValue) -> Self {
        Self(value)
    }

    pub const fn to_value(self) -> IndexValue {
        let Index(value) = self;
        value
    }
}

impl From<IndexValue> for Index {
    fn from(from: IndexValue) -> Self {
        Self::new(from)
    }
}

impl From<Index> for IndexValue {
    fn from(from: Index) -> Self {
        from.to_value()
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Prefixed with NUMBER SIGN, HASHTAG
        write!(f, "#{}", self.to_value())
    }
}

/// Measurement of a single register
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IndexedMeasurement<Value> {
    pub index: Index,
    pub measurement: Measurement<Value>,
}

/// An observation of a single register value
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ObservedValue<Value> {
    pub observed_at: SystemTimeInstant,
    pub value: Value,
}

/// A partial observation of multiple register values
///
/// The indexes of the registers are implicitly defined by their
/// order, i.e. the mapping to a register index is defined in the
/// outer context.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ObservedValues<Value> {
    pub observed_at: SystemTimeInstant,
    pub values: Vec<Option<Value>>,
}
