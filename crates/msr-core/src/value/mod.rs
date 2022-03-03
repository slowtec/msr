use std::{fmt, time::Duration};

// TODO: Make `scalar` module public instead of renaming and re-exporting all types?
mod scalar;
pub use self::scalar::{Type as ScalarType, Value as ScalarValue};

pub trait ToValueType {
    fn to_value_type(&self) -> ValueType;
}

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

// TODO: Use short identifiers?
const TYPE_STR_DURATION: &str = "duration";
const TYPE_STR_STRING: &str = "string";
const TYPE_STR_BYTES: &str = "bytes";

impl ValueType {
    #[must_use]
    pub const fn to_scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_scalar(self) -> bool {
        self.to_scalar().is_some()
    }

    #[must_use]
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

    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        ScalarType::try_from_str(s).map(Into::into).or(match s {
            TYPE_STR_DURATION => Some(Self::Duration),
            TYPE_STR_STRING => Some(Self::String),
            TYPE_STR_BYTES => Some(Self::Bytes),
            _ => None,
        })
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
///
/// TODO: Split into a separate type for simple, copyable values and
/// an enclosing type that includes the complex, non-real-time-safe
/// values?
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Scalar value (real-time safe)
    ///
    /// This variant can safely be used in real-time contexts.
    Scalar(ScalarValue),

    /// Duration, e.g. a timeout
    ///
    /// This variant can safely be used in real-time contexts.
    Duration(Duration),

    /// Variable-size text data
    ///
    /// This variant must not be used in real-time contexts.
    String(String),

    /// Variable-size binary data
    ///
    /// This variant must not be used in real-time contexts.
    Bytes(Vec<u8>),
}

impl From<Duration> for Value {
    fn from(from: Duration) -> Value {
        Self::Duration(from)
    }
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

impl Value {
    #[must_use]
    pub const fn to_type(&self) -> ValueType {
        match self {
            Self::Scalar(value) => ValueType::Scalar(value.to_type()),
            Self::Duration(_) => ValueType::Duration,
            Self::String(_) => ValueType::String,
            Self::Bytes(_) => ValueType::Bytes,
        }
    }

    #[must_use]
    pub const fn to_scalar(&self) -> Option<ScalarValue> {
        match self {
            Self::Scalar(scalar) => Some(*scalar),
            _ => None,
        }
    }

    #[must_use]
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

impl ToValueType for Value {
    fn to_value_type(&self) -> ValueType {
        self.to_type()
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
