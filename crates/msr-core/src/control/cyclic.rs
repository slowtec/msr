use std::{fmt, time::Instant};

use crate::register;

pub type CycleIdValue = u16;

/// Numeric identifier of a control cycle
///
/// Periodic control cycles are usually distinguished by their
/// frequency. This identifier allows to reference control cycles
/// independent of their actual properties.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CycleId(CycleIdValue);

impl CycleId {
    pub const fn from_value(value: CycleIdValue) -> Self {
        Self(value)
    }

    pub const fn to_value(self) -> CycleIdValue {
        self.0
    }
}

impl From<CycleIdValue> for CycleId {
    fn from(from: CycleIdValue) -> Self {
        Self::from_value(from)
    }
}

impl From<CycleId> for CycleIdValue {
    fn from(from: CycleId) -> Self {
        from.to_value()
    }
}

impl fmt::Display for CycleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.to_value())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CycleTimeStamp {
    pub id: CycleId,

    pub ts: Instant,
}

impl CycleTimeStamp {
    pub fn now(id: CycleId) -> Self {
        Self {
            id,
            ts: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CyclicRegisterMeasurements<RegisterValue> {
    pub cycle_id: CycleId,

    /// The cycle during which the measurements have been collected
    pub cycle_ts: CycleTimeStamp,

    /// Measurements for a set of registers
    ///
    /// Each register is supposed to appear at most once in the
    /// vector!
    pub registers: Vec<register::IndexedMeasurement<RegisterValue>>,
}

impl<RegisterValue> CyclicRegisterMeasurements<RegisterValue> {
    pub fn count_number_unique_of_registers(&self) -> usize {
        let mut register_indices: Vec<_> = self.registers.iter().map(|m| m.index).collect();
        register_indices.sort_unstable();
        register_indices.dedup();
        register_indices.len()
    }

    pub fn contains_duplicate_registers(&self) -> bool {
        self.registers.len() > self.count_number_unique_of_registers()
    }
}
