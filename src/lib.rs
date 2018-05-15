use std::{collections::HashMap, time::Duration};

mod entities;
mod value;

pub use self::entities::*;
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

/// The state of all inputs and outputs of a MSR system.
/// # Example
/// ```rust,no_run
/// use msr::*;
/// use std::{thread, time::Duration};
///
/// let sensor = Input::new("tcr001");
/// let actuator = Output::new("h1");
/// let mut state = IoState::default();
///
/// loop {
///     // Read some inputs (you'd use s.th. like 'read_sensor(&sensor)')
///     let sensor_value = Value::Decimal(8.9);
///     state.input.insert(&sensor, sensor_value);
///
///     // Calculate some outputs (you'd use s.th. like 'calc(&state)')
///     let actuator_value = Value::Decimal(1.7);
///     state.output.insert(&actuator, actuator_value);
///
///     // Wait for next cycle
///     thread::sleep(Duration::from_secs(2));
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IoState<'a> {
    /// Input gates (sensors)
    pub input: HashMap<&'a Input<'a>, Value>,
    /// Output gates (actuators)
    pub output: HashMap<&'a Output<'a>, Value>,
}

impl<'a> Default for IoState<'a> {
    fn default() -> Self {
        IoState {
            input: HashMap::new(),
            output: HashMap::new(),
        }
    }
}
