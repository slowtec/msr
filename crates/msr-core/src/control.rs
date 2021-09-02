use crate::Measurement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<V> {
    Input(Input<V>),
    Output(Output<V>),
}

impl<V> From<Output<V>> for Value<V> {
    fn from(from: Output<V>) -> Self {
        Self::Output(from)
    }
}

impl<V> From<Input<V>> for Value<V> {
    fn from(from: Input<V>) -> Self {
        Self::Input(from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input<V> {
    pub observed: Option<Measurement<V>>,
}

impl<V> Input<V> {
    pub const fn new() -> Self {
        Self { observed: None }
    }
}

impl<V> Default for Input<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output<V> {
    pub observed: Option<Measurement<V>>,
    pub desired: Option<Measurement<V>>,
}

impl<V> Output<V> {
    pub const fn new() -> Self {
        Self {
            observed: None,
            desired: None,
        }
    }
}

impl<V> Default for Output<V> {
    fn default() -> Self {
        Self::new()
    }
}
