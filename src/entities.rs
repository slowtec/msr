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
/// tcr003.desc = Some("This sensor measures the environment temperature".into());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    /// The unique ID of the input
    pub id: String,
    /// A more detailed description
    pub desc: Option<String>,
}

impl Input {
    pub fn new(id: String) -> Self {
        Input { id, desc: None }
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
    /// A more detailed description
    pub desc: Option<String>,
}

impl Output {
    pub fn new(id: String) -> Self {
        Output { id, desc: None }
    }
}

impl<'a> From<&'a str> for Output {
    fn from(id: &'a str) -> Self {
        Output::new(id.into())
    }
}

/// A Rule connects a condition with a list of actions
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// The unique ID of the rule
    pub id: String,
    /// A more detailed description
    pub desc: Option<String>,
    /// The condition
    pub condition: BooleanExpr<Comparison>,
    /// Actions that should be triggerd
    pub actions: Vec<String>,
}
