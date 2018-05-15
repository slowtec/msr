use std::{collections::HashMap, time::Duration};

mod value;
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
#[derive(Debug, Clone, PartialEq)]
pub struct IoState<'a> {
    /// Input gates (sensors)
    pub input: HashMap<&'a str, &'a Value>,
    /// Output gates (actuators)
    pub output: HashMap<&'a str, &'a Value>,
}

impl<'a> Default for IoState<'a> {
    fn default() -> Self {
        IoState {
            input: HashMap::new(),
            output: HashMap::new(),
        }
    }
}
