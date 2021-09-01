use std::time::Duration;

mod scalar;
pub use scalar::{Type as ScalarValueType, Value as ScalarValue};

/// Enumeration of value types
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValueType {
    /// Scalar type (real-time safe)
    Scalar(ScalarValueType),

    // Other type(s) (not real-time safe!!!)
    /// Value of e.g. a serial communication device.
    String,
    /// Binary data
    Bin,
    /// Duration (e.g. a timeout)
    Duration,
}

/// A value representation within a MSR system.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Scalar value (real-time safe)
    Scalar(ScalarValue),

    // Other type(s) (not real-time safe!!!)
    // WARNING: Use of these types is strictly forbidden in real-time contexts!
    /// Value of e.g. a serial communication device.
    String(String),
    /// Binary data
    Bin(Vec<u8>),
    /// Duration (e.g. a timeout)
    Duration(Duration),
}

impl From<bool> for Value {
    fn from(from: bool) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<i64> for Value {
    fn from(from: i64) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<u64> for Value {
    fn from(from: u64) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<f64> for Value {
    fn from(from: f64) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<i32> for Value {
    fn from(from: i32) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<u32> for Value {
    fn from(from: u32) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<f32> for Value {
    fn from(from: f32) -> Value {
        Self::Scalar(ScalarValue::from(from))
    }
}

impl From<String> for Value {
    fn from(from: String) -> Value {
        Self::String(from)
    }
}

impl From<Vec<u8>> for Value {
    fn from(from: Vec<u8>) -> Value {
        Self::Bin(from)
    }
}

impl From<Duration> for Value {
    fn from(from: Duration) -> Value {
        Self::Duration(from)
    }
}
