use super::*;

/// Input gate (sensor)
///
/// # Example
/// ```rust,no_run
/// use msr::*;
///
/// let tcr001 = Input::new("tcr001".into());
///
/// // or create it from a str
/// let tcr002 : Input = "tcr002".into();
///
/// // You can also add some description to it
/// let mut tcr003 = Input::new("tcr003".into());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    /// The unique ID of the input
    pub id: String,
    /// Value mapping
    pub mapping: Option<ValueMapping>,
}

/// Map a number **from** one range **to** another.
///
/// That is, a value of `from.low` would get mapped to `to.low`,
/// a value of `from.high` to `to.high`,
/// values in-between to values in-between, etc.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ValueMapping {
    /// The bounds of the value’s current range
    pub from: ValueBounds,
    /// The bounds of the value’s target range
    pub to: ValueBounds,
}

impl ValueMapping {
    pub fn map(&self, x: f64) -> f64 {
        util::map_value(x, self.from.low, self.from.high, self.to.low, self.to.high)
    }
}

/// Bounds of a value’s range.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ValueBounds {
    /// the lower bound of the value’s range
    pub low: f64,
    /// the upper bound of the value’s range
    pub high: f64,
}

impl Input {
    pub fn new(id: String) -> Self {
        Input { id, mapping: None }
    }
}

impl<'a> From<&'a str> for Input {
    fn from(id: &'a str) -> Self {
        Input::new(id.into())
    }
}

/// Output gate (actuator)
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// The unique ID of the output
    pub id: String,
    /// Value mapping
    pub mapping: Option<ValueMapping>,
}

impl Output {
    pub fn new(id: String) -> Self {
        Output { id, mapping: None }
    }
}

impl<'a> From<&'a str> for Output {
    fn from(id: &'a str) -> Self {
        Output::new(id.into())
    }
}

/// A loop continuously triggers a controller again and again.
#[derive(Debug, Clone)]
pub struct Loop {
    /// The unique ID of the rule
    pub id: String,
    /// Used inputs
    pub inputs: Vec<String>,
    /// Used outputs
    pub outputs: Vec<String>,
    /// The controller configuration
    pub controller: ControllerConfig,
}

/// A periodic interval with a fixed duration
#[derive(Debug, Clone)]
pub struct Interval {
    /// The unique ID of the interval
    pub id: String,
    /// The duration between two events
    pub duration: Duration,
}

/// A Rule connects a condition with a list of actions.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// The unique ID of the rule
    pub id: String,
    /// The condition
    pub condition: BooleanExpr<Comparison>,
    /// Actions that should be triggerd
    pub actions: Vec<String>,
}

/// An action can modify outputs and setpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// The unique ID of the action
    pub id: String,
    /// Define output values
    pub outputs: HashMap<String, Source>,
    /// Define memory values
    pub memory: HashMap<String, Source>,
    /// Define setpoint values
    pub setpoints: HashMap<String, Source>,
    /// Reset controller states
    pub controller_resets: Vec<String>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn map_input() {
        let map = ValueMapping {
            from: ValueBounds {
                low: 4.0,
                high: 20.0,
            },
            to: ValueBounds {
                low: 0.0,
                high: 100.0,
            },
        };
        assert_eq!(map.map(4.0), 0.0);
        assert_eq!(map.map(12.0), 50.0);
        assert_eq!(map.map(20.0), 100.0);
    }
}
