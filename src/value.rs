/// A value representation within a MSR system.
#[derive(Debug, Clone, PartialEq)]
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
}
