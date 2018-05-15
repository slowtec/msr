use std::time::Duration;

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
