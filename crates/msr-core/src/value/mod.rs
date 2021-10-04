use std::{fmt, time::Duration};

mod scalar;
pub use self::scalar::{Type as ScalarType, Value as ScalarValue};

/// Enumeration of value types
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValueType {
    /// Scalar type
    Scalar(ScalarType),

    /// Time duration, e.g. a timeout
    Duration,

    /// Text data
    String,

    /// Binary data
    Bytes,
}

const TYPE_STR_DURATION: &str = "duration";
const TYPE_STR_STRING: &str = "string";
const TYPE_STR_BYTES: &str = "bytes";

impl ValueType {
    pub const fn to_scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar(s) => Some(s),
            _ => None,
        }
    }

    pub const fn is_scalar(self) -> bool {
        self.to_scalar().is_some()
    }

    pub const fn from_scalar(scalar: ScalarType) -> Self {
        Self::Scalar(scalar)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Scalar(s) => s.as_str(),
            Self::Duration => TYPE_STR_DURATION,
            Self::String => TYPE_STR_STRING,
            Self::Bytes => TYPE_STR_BYTES,
        }
    }
}

impl From<ScalarType> for ValueType {
    fn from(from: ScalarType) -> Self {
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

    /// Duration, e.g. a timeout
    Duration(Duration),

    /// Variable-size text data
    ///
    /// Should not be used in real-time contexts!
    String(String),

    /// Variable-size binary data
    ///
    /// Should not be used in real-time contexts!
    Bytes(Vec<u8>),
}

impl From<String> for Value {
    fn from(from: String) -> Value {
        Self::String(from)
    }
}

impl From<Vec<u8>> for Value {
    fn from(from: Vec<u8>) -> Value {
        Self::Bytes(from)
    }
}

impl From<Duration> for Value {
    fn from(from: Duration) -> Value {
        Self::Duration(from)
    }
}

impl Value {
    pub const fn to_type(&self) -> ValueType {
        match self {
            Self::Scalar(value) => ValueType::Scalar(value.to_type()),
            Self::Duration(_) => ValueType::Duration,
            Self::String(_) => ValueType::String,
            Self::Bytes(_) => ValueType::Bytes,
        }
    }

    pub const fn to_scalar(&self) -> Option<ScalarValue> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            _ => None,
        }
    }

    pub const fn from_scalar(scalar: ScalarValue) -> Self {
        Self::Scalar(scalar)
    }

    pub fn to_i32(&self) -> Option<i32> {
        self.to_scalar().and_then(ScalarValue::to_i32)
    }

    pub fn to_u32(&self) -> Option<u32> {
        self.to_scalar().and_then(ScalarValue::to_u32)
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.to_scalar().and_then(ScalarValue::to_i64)
    }

    pub fn to_u64(&self) -> Option<u64> {
        self.to_scalar().and_then(ScalarValue::to_u64)
    }

    pub fn to_f32(&self) -> Option<f32> {
        self.to_scalar().and_then(ScalarValue::to_f32)
    }

    pub fn to_f64(&self) -> Option<f64> {
        self.to_scalar().and_then(ScalarValue::to_f64)
    }
}

impl From<Value> for ValueType {
    fn from(from: Value) -> Self {
        from.to_type()
    }
}

impl<S> From<S> for Value
where
    S: Into<ScalarValue>,
{
    fn from(from: S) -> Self {
        Value::Scalar(from.into())
    }
}
