/// Input gate (sensor)
///
/// # Example
/// ```rust,no_run
/// use msr::*;
///
/// let tcr001 = Input::new("tcr001");
///
/// // or create it from a str
/// let tcr002 : Input = "tcr002".into();
///
/// // You can also add some description to it
/// let mut tcr003 = Input::new("tcr003");
/// tcr003.desc = Some("This sensor measures the environment temperature");
/// ```
#[derive(Debug, Eq)]
pub struct Input<'a> {
    /// The unique ID of the input
    pub id: &'a str,
    /// A more detailed description
    pub desc: Option<&'a str>,
}

impl<'a> Input<'a> {
    pub fn new(id: &'a str) -> Self {
        Input { id, desc: None }
    }
}

impl<'a> From<&'a str> for Input<'a> {
    fn from(id: &'a str) -> Self {
        Input::new(id)
    }
}

impl<'a> PartialEq for Input<'a> {
    fn eq(&self, other: &Input) -> bool {
        self.id == other.id
    }
}

use std::hash::{Hash, Hasher};

impl<'a> Hash for Input<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        "in".hash(state);
    }
}

/// Output gate (actuator)
#[derive(Debug, Eq)]
pub struct Output<'a> {
    /// The unique ID of the output
    pub id: &'a str,
    /// A more detailed description
    pub desc: Option<&'a str>,
}

impl<'a> Output<'a> {
    pub fn new(id: &'a str) -> Self {
        Output { id, desc: None }
    }
}

impl<'a> From<&'a str> for Output<'a> {
    fn from(id: &'a str) -> Self {
        Output::new(id)
    }
}

impl<'a> PartialEq for Output<'a> {
    fn eq(&self, other: &Output) -> bool {
        self.id == other.id
    }
}

impl<'a> Hash for Output<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        "out".hash(state);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    #[test]
    fn input_equality() {
        let a0 = Input {
            id: "a",
            desc: None,
        };
        let a1 = Input {
            id: "a",
            desc: Some("foo"),
        };
        assert!(a0.eq(&a1));
    }

    #[test]
    fn output_equality() {
        let o0 = Output {
            id: "a",
            desc: None,
        };
        let o1 = Output {
            id: "a",
            desc: Some("foo"),
        };
        assert!(o0.eq(&o1));
    }

    #[test]
    fn input_hash() {
        let a0 = Input {
            id: "a",
            desc: None,
        };
        let a1 = Input {
            id: "a",
            desc: Some("foo"),
        };
        let mut h0 = DefaultHasher::new();
        let mut h1 = DefaultHasher::new();
        a0.hash(&mut h0);
        a1.hash(&mut h1);
        assert_eq!(h0.finish(), h1.finish());
    }

    #[test]
    fn output_hash() {
        let o0 = Output {
            id: "a",
            desc: None,
        };
        let o1 = Output {
            id: "a",
            desc: Some("foo"),
        };
        let mut h0 = DefaultHasher::new();
        let mut h1 = DefaultHasher::new();
        o0.hash(&mut h0);
        o1.hash(&mut h1);
        assert_eq!(h0.finish(), h1.finish());
    }

    #[test]
    fn in_and_output_hash() {
        let o = Output {
            id: "a",
            desc: None,
        };
        let i = Input {
            id: "a",
            desc: None,
        };
        let mut h0 = DefaultHasher::new();
        let mut h1 = DefaultHasher::new();
        o.hash(&mut h0);
        i.hash(&mut h1);
        assert!(h0.finish() != h1.finish());
    }
}
