use std::{
    collections::HashMap, io::{Error, ErrorKind, Result}, time::Duration,
};

mod comparison;
mod entities;
mod parser;
mod runtime;
pub mod util;
mod value;

pub use self::{comparison::*, entities::*, runtime::*, value::*};

/// PID controller
pub mod pid;

/// Bang-bang controller
pub mod bang_bang;

/// A generic statefull controller
pub trait Controller<Input, Output> {
    /// Calculate the next state.
    fn next(&mut self, input: Input) -> Output;
}

/// A generic stateless controller
pub trait PureController<Input, Output> {
    /// Calculate the next state.
    fn next(&self, input: Input) -> Output;
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
#[derive(Debug, Clone)]
pub enum ControllerType {
    Pid(pid::Pid),
    BangBang(bang_bang::BangBang),
}

/// Controller configuration
#[derive(Debug, Clone)]
pub enum ControllerConfig {
    Pid(pid::PidConfig),
    BangBang(bang_bang::BangBangConfig),
}

/// Controller state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControllerState {
    Pid(pid::PidState),
    BangBang(bang_bang::BangBangState),
}

impl<'a>
    PureController<
        (&'a ControllerState, &'a IoState, &'a Duration),
        Result<(ControllerState, IoState)>,
    > for Loop
{
    fn next(
        &self,
        input: (&ControllerState, &IoState, &Duration),
    ) -> Result<(ControllerState, IoState)> {
        let (controller, io, dt) = input;
        if self.inputs.len() != 1 || self.outputs.len() != 1 {
            return Err(Error::new(
                ErrorKind::Other,
                "Loop has invalid length of inputs/outputs",
            ));
        }

        let input_id = &self.inputs[0];

        if let Some(Value::Decimal(v)) = io.inputs.get(input_id) {
            let mut io = io.clone();
            let output_id = self.outputs[0].clone();

            match self.controller {
                ControllerConfig::Pid(ref cfg) => match controller {
                    ControllerState::Pid(s) => {
                        let (pid_state, y) = cfg.next((*s, *v, dt));
                        io.outputs.insert(output_id, y.into());
                        let controller = ControllerState::Pid(pid_state);
                        Ok((controller, io))
                    }
                    _ => Err(Error::new(
                        ErrorKind::InvalidData,
                        "Invalid controller state: a PID state is is required",
                    )),
                },
                ControllerConfig::BangBang(ref cfg) => match controller {
                    ControllerState::BangBang(s) => {
                        let bb_state = cfg.next((*s, *v));
                        io.outputs.insert(output_id, bb_state.into());
                        let controller = ControllerState::BangBang(bb_state);
                        Ok((controller, io))
                    }
                    _ => Err(Error::new(
                        ErrorKind::InvalidData,
                        "Invalid controller state: a BangBang state is is required",
                    )),
                },
            }
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid input data type: a decimal value is required",
            ))
        }
    }
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
///     state.inputs.insert("tcr001".into(), sensor_value);
///
///     // Calculate some outputs (you'd use s.th. like 'calc(&state)')
///     let actuator_value = Value::Decimal(1.7);
///     state.outputs.insert("h1".into(), actuator_value);
///
///     // Wait for next cycle
///     thread::sleep(Duration::from_secs(2));
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IoState {
    /// Input gates (sensors)
    pub inputs: HashMap<String, Value>,
    /// Output gates (actuators)
    pub outputs: HashMap<String, Value>,
}

impl Default for IoState {
    fn default() -> Self {
        IoState {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        }
    }
}

impl SyncIoSystem for IoState {
    fn read(&mut self, id: &str) -> Result<Value> {
        Ok(self
            .inputs
            .get(id)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "no such input"))?
            .clone())
    }

    fn read_output(&mut self, id: &str) -> Result<Option<Value>> {
        Ok(self.outputs.get(id).cloned())
    }

    fn write(&mut self, id: &str, v: &Value) -> Result<()> {
        self.outputs.insert(id.into(), v.clone());
        Ok(())
    }
}

/// A data source
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Source {
    In(String),
    Out(String),
    Const(Value),
}

impl Source {
    pub fn cmp_eq(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::Equal)
    }
    pub fn cmp_le(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::LessOrEqual)
    }
    pub fn cmp_ge(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::GreaterOrEqual)
    }
    pub fn cmp_ne(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::NotEqual)
    }
    pub fn cmp_lt(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::Less)
    }
    pub fn cmp_gt(self, right: Source) -> Comparison {
        self.cmp(right, Comparator::Greater)
    }
    fn cmp(self, right: Source, cmp: Comparator) -> Comparison {
        Comparison {
            left: self,
            cmp,
            right,
        }
    }
}

/// A boolean expression
#[derive(Debug, Clone, PartialEq)]
pub enum BooleanExpr<T> {
    /// `true`
    True,
    /// `false`
    False,
    /// The logical AND of two expressions.
    And(Box<BooleanExpr<T>>, Box<BooleanExpr<T>>),
    /// The locigal OR of two expressions.
    Or(Box<BooleanExpr<T>>, Box<BooleanExpr<T>>),
    /// The logical complement of the contained expression.
    Not(Box<BooleanExpr<T>>),
    /// Evaluate expr of type `T`
    /// This expression represents a value that is not known until evaluation time.
    Eval(T),
}

/// A condition that can be evaulated with a given [IoState]
pub trait IoCondition {
    fn eval(&self, io: &IoState) -> Result<bool>;
}

trait IoSources {
    fn sources(&self) -> Vec<Source>;
}

impl IoSources for BooleanExpr<Comparison> {
    fn sources(&self) -> Vec<Source> {
        use BooleanExpr::*;
        match self {
            And(ref a, ref b) | Or(ref a, ref b) => {
                let mut srcs = a.sources();
                srcs.append(&mut b.sources());
                srcs
            }
            Not(ref x) => x.sources(),
            Eval(ref x) => vec![x.left.clone(), x.right.clone()],
            True | False => vec![],
        }
    }
}

impl<T> IoCondition for BooleanExpr<T>
where
    T: IoCondition,
{
    fn eval(&self, io: &IoState) -> Result<bool> {
        use BooleanExpr::*;
        match self {
            True => Ok(true),
            False => Ok(false),
            And(ref a, ref b) => Ok(a.eval(io)? && b.eval(io)?),
            Or(ref a, ref b) => Ok(a.eval(io)? || b.eval(io)?),
            Not(ref x) => Ok(!x.eval(io)?),
            Eval(ref x) => x.eval(io),
        }
    }
}

impl<T> From<T> for Source
where
    T: Into<Value>,
{
    fn from(x: T) -> Source {
        Source::Const(x.into())
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
        io.inputs.insert("foo".into(), Value::Bit(true));
        assert_eq!(io.read("foo").unwrap(), Value::Bit(true));
    }

    #[test]
    fn bool_expr_eval() {
        use BooleanExpr::*;
        use Source::*;

        let mut io = IoState::default();

        // x > 5.0
        let x_gt_5 = In("x".into()).cmp_gt(5.0.into());
        let expr = Eval(x_gt_5.clone());
        io.inputs.insert("x".into(), 5.0.into());
        assert_eq!(expr.eval(&mut io).unwrap(), false);

        // y == true
        let y_eq_true = In("y".into()).cmp_eq(true.into());

        // x > 5.0 && y == true
        let expr = And(
            Box::new(Eval(x_gt_5.clone())),
            Box::new(Eval(y_eq_true.clone())),
        );
        io.inputs.insert("x".into(), 5.1.into());
        io.inputs.insert("y".into(), true.into());
        assert_eq!(expr.eval(&mut io).unwrap(), true);
        io.inputs.insert("y".into(), false.into());
        assert_eq!(expr.eval(&mut io).unwrap(), false);

        // x > 5.0 || y == true
        let expr = Or(
            Box::new(Eval(x_gt_5.clone())),
            Box::new(Eval(y_eq_true.clone())),
        );
        io.inputs.insert("x".into(), 3.0.into());
        io.inputs.insert("y".into(), true.into());
        assert_eq!(expr.eval(&mut io).unwrap(), true);
        io.inputs.insert("y".into(), false.into());
        assert_eq!(expr.eval(&mut io).unwrap(), false);

        // !(x > 5.0)
        let expr = Not(Box::new(Eval(x_gt_5)));
        io.inputs.insert("x".into(), 6.0.into());
        assert_eq!(expr.eval(&mut io).unwrap(), false);

        // just true
        let expr: BooleanExpr<Comparison> = True;
        assert_eq!(expr.eval(&mut io).unwrap(), true);
    }

    #[test]
    fn bool_expr_sources() {
        use BooleanExpr::*;
        use Source::*;

        let x_gt_5 = In("x".into()).cmp_gt(5.0.into());
        let expr = Eval(x_gt_5.clone());
        assert_eq!(expr.sources(), vec![In("x".into()), Const(5.0.into())]);

        let y_eq_z = Out("y".into()).cmp_eq(In("z".into()));
        let expr = And(Box::new(Eval(x_gt_5)), Box::new(Eval(y_eq_z)));
        assert_eq!(
            expr.sources(),
            vec![
                In("x".into()),
                Const(5.0.into()),
                Out("y".into()),
                In("z".into()),
            ]
        );
    }

    #[test]
    fn pure_pid_loop() {
        let mut pid_cfg = pid::PidConfig::default();
        pid_cfg.k_p = 2.0;
        let l = Loop {
            id: "pid".into(),
            desc: None,
            inputs: vec!["x".into()],
            outputs: vec!["y".into()],
            controller: ControllerConfig::Pid(pid_cfg),
        };
        let mut io = IoState::default();
        io.inputs.insert("x".into(), 140.0.into());
        let mut pid_state = pid::PidState::default();
        pid_state.target = 150.0;
        let controller = ControllerState::Pid(pid_state);
        let dt = Duration::from_secs(1);
        let (c, io) = l.next((&controller, &io, &dt)).unwrap();
        assert_eq!(*io.outputs.get("y").unwrap(), Value::Decimal(20.0));
        match c {
            ControllerState::Pid(pid) => {
                assert_eq!(pid.prev_value, Some(140.0));
            }
            _ => {
                panic!("invalid controller state");
            }
        }
    }

    #[test]
    fn pure_bb_loop() {
        let mut bb_cfg = bang_bang::BangBangConfig::default();
        bb_cfg.threshold = 5.0;
        let l = Loop {
            id: "bb".into(),
            desc: None,
            inputs: vec!["x".into()],
            outputs: vec!["y".into()],
            controller: ControllerConfig::BangBang(bb_cfg),
        };
        let mut io = IoState::default();
        io.inputs.insert("x".into(), 5.1.into());
        let controller = ControllerState::BangBang(false);
        let dt = Duration::from_secs(1);
        let (_, io) = l.next((&controller, &io, &dt)).unwrap();
        assert_eq!(*io.outputs.get("y").unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_loops_inputs_and_outputs_len() {
        let controller = ControllerConfig::BangBang(bang_bang::BangBangConfig::default());
        let dt = Duration::from_millis(5);
        let mut loop0 = Loop {
            id: "foo".into(),
            desc: None,
            inputs: vec![],
            outputs: vec![],
            controller,
        };
        let mut io = IoState::default();
        io.inputs.insert("input".into(), 0.0.into());
        let controller = ControllerState::BangBang(bang_bang::BangBangState::default());
        assert!(loop0.next((&controller, &io, &dt)).is_err());
        loop0.inputs = vec!["input".into()];
        assert!(loop0.next((&controller, &io, &dt)).is_err());
        loop0.outputs = vec!["output".into()];
        assert!(loop0.next((&controller, &io, &dt)).is_ok());
    }
}
