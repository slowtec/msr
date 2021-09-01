use std::{fmt, time::Duration};

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

const TYPE_STR_STRING: &str = "string";
const TYPE_STR_BIN: &str = "binary";
const TYPE_STR_DURATION: &str = "duration";

impl ValueType {
    pub const fn as_scalar(self) -> Option<ScalarValueType> {
        match self {
            Self::Scalar(s) => Some(s),
            _ => None,
        }
    }

    pub const fn is_scalar(self) -> bool {
        self.as_scalar().is_some()
    }

    pub const fn from_scalar(scalar: ScalarValueType) -> Self {
        Self::Scalar(scalar)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Scalar(s) => s.as_str(),
            Self::String => TYPE_STR_STRING,
            Self::Bin => TYPE_STR_BIN,
            Self::Duration => TYPE_STR_DURATION,
        }
    }
}

impl From<ScalarValueType> for ValueType {
    fn from(from: ScalarValueType) -> Self {
        Self::from_scalar(from)
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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

impl Value {
    pub const fn value_type(&self) -> ValueType {
        match self {
            Self::Scalar(s) => ValueType::Scalar(s.value_type()),
            Self::String(_) => ValueType::String,
            Self::Bin(_) => ValueType::Bin,
            Self::Duration(_) => ValueType::Duration,
        }
    }

    pub fn to_scalar(&self) -> Option<scalar::Value> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            _ => None,
        }
    }

    pub fn to_i32(&self) -> Option<i32> {
        self.to_scalar().and_then(scalar::Value::to_i32)
    }

    pub fn to_u32(&self) -> Option<u32> {
        self.to_scalar().and_then(scalar::Value::to_u32)
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.to_scalar().and_then(scalar::Value::to_i64)
    }

    pub fn to_u64(&self) -> Option<u64> {
        self.to_scalar().and_then(scalar::Value::to_u64)
    }

    pub fn to_f32(&self) -> Option<f32> {
        self.to_scalar().and_then(scalar::Value::to_f32)
    }

    pub fn to_f64(&self) -> Option<f64> {
        self.to_scalar().and_then(scalar::Value::to_f64)
    }
}

impl From<Value> for ValueType {
    fn from(from: Value) -> Self {
        from.value_type()
    }
}

impl<S> From<S> for Value
where
    S: Into<scalar::Value>,
{
    fn from(from: S) -> Self {
        Value::Scalar(from.into())
    }
}
