use std::{
    collections::HashMap, io::{Error, ErrorKind, Result}, time::Duration,
};

mod entities;
mod runtime;
mod value;

pub use self::entities::*;
pub use self::runtime::*;
pub use self::value::*;

/// PID controller
pub mod pid;

/// Bang-bang controller
pub mod bang_bang;

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

/// An I/O system with synchronous fieldbus access
pub trait SyncIoSystem {
    /// Read the current state of an input.
    fn read(&mut self, id: &str) -> Result<Value>;
    /// Read the current state of an output if possible.
    fn read_output(&mut self, id: &str) -> Result<Option<Value>>;
    /// Write a value to the specified output.
    fn write(&mut self, id: &str, value: &Value) -> Result<()>;
}

/// Controller type
pub enum ControllerType {
    Pid(pid::Pid),
    BangBang(bang_bang::BangBang),
}

/// A loop contiuously triggers a controller again and again.
pub struct Loop {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub controller: ControllerType,
}

/// The state of all inputs and outputs of a MSR system.
/// # Example
/// ```rust,no_run
/// use msr::*;
/// use std::{thread, time::Duration};
///
/// let mut state = IoState::default();
///
/// loop {
///     // Read some inputs (you'd use s.th. like 'read("sensor_id")')
///     let sensor_value = Value::Decimal(8.9);
///     state.input.insert("tcr001".into(), sensor_value);
///
///     // Calculate some outputs (you'd use s.th. like 'calc(&state)')
///     let actuator_value = Value::Decimal(1.7);
///     state.output.insert("h1".into(), actuator_value);
///
///     // Wait for next cycle
///     thread::sleep(Duration::from_secs(2));
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IoState {
    /// Input gates (sensors)
    pub input: HashMap<String, Value>,
    /// Output gates (actuators)
    pub output: HashMap<String, Value>,
}

impl Default for IoState {
    fn default() -> Self {
        IoState {
            input: HashMap::new(),
            output: HashMap::new(),
        }
    }
}

impl SyncIoSystem for IoState {
    fn read(&mut self, id: &str) -> Result<Value> {
        Ok(self
            .input
            .get(id)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "no such input"))?
            .clone())
    }

    fn read_output(&mut self, id: &str) -> Result<Option<Value>> {
        Ok(self.output.get(id).cloned())
    }

    fn write(&mut self, id: &str, v: &Value) -> Result<()> {
        self.output.insert(id.into(), v.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn io_state_as_sync_io_system() {
        let mut io = IoState::default();
        assert!(io.read("foo").is_err());
        assert!(io.read_output("foo").unwrap().is_none());
        assert!(io.write("foo", &Value::Decimal(3.3)).is_ok());
        assert!(io.read("foo").is_err());
        assert_eq!(io.read_output("foo").unwrap(), Some(Value::Decimal(3.3)));
        io.input.insert("foo".into(), Value::Bit(true));
        assert_eq!(io.read("foo").unwrap(), Value::Bit(true));
    }
}
