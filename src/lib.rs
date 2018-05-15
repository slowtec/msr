use std::time::Duration;

/// PID controller
pub mod pid;

/// Bang-bang controller
pub mod bang_bang;

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

/// A generic statefull controller
pub trait Controller<Input, Output> {
    /// Calculate the next state.
    fn next(&mut self, input: Input) -> Output;
}

/// A generic statefull controller with time steps
pub trait TimeStepController<Input, Output> {
    /// Calculate the next state.
    fn next(&mut self, input: Input, delta_t: &Duration) -> Output;
}

impl<I, O, C> TimeStepController<I, O> for C
where
    for<'a> C: Controller<(I, &'a Duration), O>,
{
    fn next(&mut self, input: I, delta_t: &Duration) -> O {
        (self as &mut Controller<(I, &Duration), O>).next((input, delta_t))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn value_casting() {
        assert_eq!(Value::from(true), Value::Bit(true));
        assert_eq!(Value::from(3.2), Value::Decimal(3.2));
        assert_eq!(Value::from(3), Value::Integer(3));
        assert_eq!(
            Value::from("txt".to_string()),
            Value::Text("txt".to_string())
        );
        assert_eq!(Value::from(vec![0x07]), Value::Bin(vec![0x07]));
    }
}
