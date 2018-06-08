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

/// Comperators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparator {
    /// `<` or `LT` (Less Than)
    Less,
    /// `<=` or `LE` (Less Than or Equal)
    LessOrEqual,
    /// `>` or `GT` (Greater Than)
    Greater,
    /// `>=` or `GE` (Greater Than or Equal)
    GreaterOrEqual,
    /// `==` or `EQ` (Equal)
    Equal,
    /// `!=` or `NE` (Not Equal)
    NotEqual,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    left: Source,
    cmp: Comparator,
    right: Source,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BooleanExpr<T>
{
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
    fn create_comparison_from_value_source() {
        let input = Source::In("x".into());
        let val = Source::Const(Value::Decimal(90.0));

        let eq = input.clone().cmp_eq(val.clone());
        assert_eq!(eq.left, input);
        assert_eq!(eq.right, val);
        assert_eq!(eq.cmp, Comparator::Equal);

        let le = input.clone().cmp_le(val.clone());
        assert_eq!(le.left, input);
        assert_eq!(le.right, val);
        assert_eq!(le.cmp, Comparator::LessOrEqual);

        let ge = input.clone().cmp_ge(val.clone());
        assert_eq!(ge.left, input);
        assert_eq!(ge.right, val);
        assert_eq!(ge.cmp, Comparator::GreaterOrEqual);

        let ne = input.clone().cmp_ne(val.clone());
        assert_eq!(ne.left, input);
        assert_eq!(ne.right, val);
        assert_eq!(ne.cmp, Comparator::NotEqual);

        let lt = input.clone().cmp_lt(val.clone());
        assert_eq!(lt.left, input);
        assert_eq!(lt.right, val);
        assert_eq!(lt.cmp, Comparator::Less);

        let gt = input.clone().cmp_gt(val.clone());
        assert_eq!(gt.left, input);
        assert_eq!(gt.right, val);
        assert_eq!(gt.cmp, Comparator::Greater);
    }
}
