use std::time::Duration;

#[cfg(feature = "serde")]
use std::fmt;

#[cfg(feature = "serde")]
use serde::{
    de::{Error, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

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

#[cfg(feature = "serde")]
struct ValueVisitor;

#[cfg(feature = "serde")]
impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a boolean, a number, a string, an array of bytes or an timeout.")
    }
    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bit(value))
    }
    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(Value::Decimal(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Integer(value))
    }
    fn visit_str<E>(self, value: &str) -> Result<Value, E>
    where
        E: ::serde::de::Error,
    {
        Ok(Value::Text(value.into()))
    }
    fn visit_seq<A>(self, mut access: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut bin: Vec<u8> = vec![];

        while let Some(value) = access.next_element()? {
            bin.push(value);
        }
        Ok(Value::Bin(bin))
    }
    fn visit_map<A>(self, mut access: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
        A::Error: ::serde::de::Error,
    {
        let mut secs: Option<u64> = None;
        let mut nanos: Option<u32> = None;

        while let Some((key, value)) = access.next_entry()? {
            match key {
                "secs" => {
                    secs = Some(value);
                }
                "nanos" => {
                    nanos = Some(value as u32);
                }
                k => return Err(A::Error::custom(format!("Unknown key: {}", k))),
            }
        }
        if let Some(secs) = secs {
            if let Some(nanos) = nanos {
                return Ok(Value::Timeout(Duration::new(secs, nanos)));
            }
        }
        Err(A::Error::custom("Unknown map"))
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[cfg(feature = "serde")]
    #[test]
    fn value_deserialization() {
        let v: Value = serde_json::from_str("true").unwrap();
        assert_eq!(v, Value::Bit(true));

        let v: Value = serde_json::from_str("6.99").unwrap();
        assert_eq!(v, Value::Decimal(6.99));

        let v: Value = serde_json::from_str("-8").unwrap();
        assert_eq!(v, Value::Integer(-8));

        let v: Value = serde_json::from_str("\"blabla\"").unwrap();
        assert_eq!(v, Value::Text("blabla".into()));

        let v: Value = serde_json::from_str("[69,255]").unwrap();
        assert_eq!(v, Value::Bin(vec![0x45, 0xFF]));

        let v: Value = serde_json::from_str("{\"secs\":1,\"nanos\":500000000}").unwrap();
        assert_eq!(v, Value::Timeout(Duration::from_millis(1500)));

        assert!(serde_json::from_str::<Value>("{\"secs\":1,\"nanooos\":500}").is_err());
        assert!(serde_json::from_str::<Value>("{\"secs\":1}").is_err());
    }
}
