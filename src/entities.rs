use super::*;
use std::time::Duration;

/// I/O (sensor or actuator)
///
/// # Example
/// ```rust,no_run
/// use msr::*;
///
/// let tcr001 = IoGate::new("tcr001".into());
///
/// // or create it from a str
/// let tcr002 : IoGate = "tcr002".into();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IoGate {
    /// The unique ID of the gate
    pub id: String,
    /// Value mapping
    pub mapping: Option<ValueMapping>,
    /// Value cropping
    pub cropping: Option<Cropping>,
    /// Value calibration
    pub calib: Option<Calibration>,
}

/// Map a number **from** one range **to** another.
///
/// That is, a value of `from.low` would get mapped to `to.low`,
/// a value of `from.high` to `to.high`,
/// values in-between to values in-between, etc.
// TODO: Make this more gerneric
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
    /// The lower bound of the value’s range
    pub low: f64,
    /// The upper bound of the value’s range
    pub high: f64,
}

/// Cropping value
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Cropping {
    /// The lower threshold
    pub low: Option<f64>,
    /// The upper threshold
    pub high: Option<f64>,
}

impl Cropping {
    /// Crop a value
    ///
    /// e.g. with `low = 2.0`
    /// - `1.9` -> `2.0`
    /// - `2.0` -> `2.0`
    ///
    /// with `high = 2.0`
    /// - `2.1` -> `2.0`
    /// - `1.9` -> `1.9`
    pub fn crop(&self, x: f64) -> f64 {
        util::limit(self.low, self.high, x)
    }
}

/// Calibration coefficients
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Calibration {
    pub a: Option<f64>,
    pub b: Option<f64>,
    pub c: Option<f64>,
}

impl IoGate {
    pub fn new(id: String) -> Self {
        IoGate {
            id,
            mapping: None,
            cropping: None,
            calib: None,
        }
    }
}

impl<'a> From<&'a str> for IoGate {
    fn from(id: &'a str) -> Self {
        IoGate::new(id.into())
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
    /// Define timeouts
    pub timeouts: HashMap<String, Option<Duration>>,
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

    #[test]
    fn crop_input() {
        let cropping = Cropping {
            low: Some(2.0),
            high: Some(3.0),
        };
        assert_eq!(cropping.crop(1.9), 2.0);
        assert_eq!(cropping.crop(-1.9), 2.0);
        assert_eq!(cropping.crop(2.0), 2.0);
        assert_eq!(cropping.crop(2.1), 2.1);
        assert_eq!(cropping.crop(3.0), 3.0);
        assert_eq!(cropping.crop(3.1), 3.0);
    }
}
