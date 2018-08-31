#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};
use std::time::Duration;

/// A value representation within a MSR system.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    /// State of e.g. a digital input/output.
    Bit(bool),
    /// Value of e.g. an analog input/output.
    Decimal(f64),
    /// Value of e.g. a counter input.
    Integer(i64),
    /// Value of e.g. a serial communication device.
    Text(String),
    /// Binary data
    Bin(Vec<u8>),
    /// Timeout
    Timeout(Duration),
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bit(b)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Value {
        Value::Decimal(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Value {
        Value::Integer(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Value {
        Value::Integer(v as i64)
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Value {
        Value::Integer(v as i64)
    }
}

impl From<String> for Value {
    fn from(t: String) -> Value {
        Value::Text(t)
    }
}

impl From<Vec<u8>> for Value {
    fn from(b: Vec<u8>) -> Value {
        Value::Bin(b)
    }
}

impl From<Duration> for Value {
    fn from(d: Duration) -> Value {
        Value::Timeout(d)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Bit(b) => serializer.serialize_bool(*b),
            Value::Decimal(d) => serializer.serialize_f64(*d),
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Text(t) => serializer.serialize_str(t),
            Value::Bin(b) => serializer.serialize_bytes(b),
            Value::Timeout(t) => t.serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[cfg(feature = "serde")]
    use serde_json;

    #[test]
    fn value_casting() {
        assert_eq!(Value::from(true), Value::Bit(true));
        assert_eq!(Value::from(3.2_f64), Value::Decimal(3.2));
        assert_eq!(Value::from(3_i64), Value::Integer(3));
        assert_eq!(Value::from(3_i32), Value::Integer(3));
        assert_eq!(Value::from(3_u32), Value::Integer(3));
        assert_eq!(
            Value::from("txt".to_string()),
            Value::Text("txt".to_string())
        );
        assert_eq!(Value::from(vec![0x07]), Value::Bin(vec![0x07]));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn value_serialization() {
        let b = Value::Bit(true);
        assert_eq!(serde_json::to_string(&b).unwrap(), "true");

        let f = Value::Decimal(6.99);
        assert_eq!(serde_json::to_string(&f).unwrap(), "6.99");

        let i = Value::Integer(-8);
        assert_eq!(serde_json::to_string(&i).unwrap(), "-8");

        let t = Value::Text("blabla".into());
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"blabla\"");

        let b = Value::Bin(vec![0x45, 0xFF]);
        assert_eq!(serde_json::to_string(&b).unwrap(), "[69,255]");

        let t = Value::Timeout(Duration::from_millis(1500));
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            "{\"secs\":1,\"nanos\":500000000}"
        );
    }
}
