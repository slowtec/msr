use std::{
    fmt,
    time::{Duration, Instant},
};

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
    #[must_use]
    pub const fn from_value(value: CycleIdValue) -> Self {
        Self(value)
    }

    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn count_number_unique_of_registers(&self) -> usize {
        let mut register_indices: Vec<_> = self.registers.iter().map(|m| m.index).collect();
        register_indices.sort_unstable();
        register_indices.dedup();
        register_indices.len()
    }

    #[must_use]
    pub fn contains_duplicate_registers(&self) -> bool {
        self.registers.len() > self.count_number_unique_of_registers()
    }
}

/// Adjust the expected cycle start time by skipping missed cycles
///
/// Adjust the expected start time of the current cycle to the
/// deadline of the previous cycle depending on the cycle time.
///
/// The function either returns the unmodified expected start time
/// or otherwise the adjusted expected start time together with the
/// number of missed cycles that have been skipped.
pub fn skip_missed_cycles(
    cycle_time: Duration,
    expected_cycle_start: Instant,
    actual_cycle_start: Instant,
) -> Result<Instant, (Instant, u32)> {
    debug_assert!(cycle_time > Duration::ZERO);
    if expected_cycle_start >= actual_cycle_start {
        return Ok(expected_cycle_start);
    }
    let elapsed_cycles = actual_cycle_start
        .duration_since(expected_cycle_start)
        .as_secs_f64()
        / cycle_time.as_secs_f64();
    debug_assert!(elapsed_cycles > 0.0);
    if elapsed_cycles < 1.0 {
        return Ok(expected_cycle_start);
    }
    // We missed at least 1 entire cycle
    let missed_cycles = elapsed_cycles.floor();
    debug_assert!(missed_cycles <= f64::from(u32::MAX));
    #[allow(clippy::cast_sign_loss)]
    let missed_cycles = missed_cycles.min(f64::from(u32::MAX)) as u32;
    // Adjust the deadline of the previous cycle
    let skipped_cycles_duration = missed_cycles * cycle_time;
    Err((
        expected_cycle_start + skipped_cycles_duration,
        missed_cycles,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)] // TODO
    fn should_skip_missed_cycles() {
        let cycle_time = Duration::from_millis(2);
        let half_cycle_time = cycle_time / 2;
        let quarter_cycle_time = cycle_time / 4;
        let three_quarter_cycle_time = half_cycle_time + quarter_cycle_time;
        let actual_cycle_start = Instant::now();

        // Exact
        assert_eq!(
            Ok(actual_cycle_start),
            skip_missed_cycles(cycle_time, actual_cycle_start, actual_cycle_start),
        );

        // Earlier than expected
        assert_eq!(
            Ok(actual_cycle_start + 100 * cycle_time),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start + 100 * cycle_time,
                actual_cycle_start
            ),
        );

        // Less than 1 cycle later as expected
        assert_eq!(
            Ok(actual_cycle_start
                .checked_sub(Duration::from_nanos(1))
                .unwrap()),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start
                    .checked_sub(Duration::from_nanos(1))
                    .unwrap(),
                actual_cycle_start
            ),
        );
        assert_eq!(
            Ok(actual_cycle_start.checked_sub(quarter_cycle_time).unwrap()),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start.checked_sub(quarter_cycle_time).unwrap(),
                actual_cycle_start
            ),
        );
        assert_eq!(
            Ok(actual_cycle_start.checked_sub(half_cycle_time).unwrap()),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start.checked_sub(half_cycle_time).unwrap(),
                actual_cycle_start
            ),
        );
        assert_eq!(
            Ok(actual_cycle_start
                .checked_sub(three_quarter_cycle_time)
                .unwrap()),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start
                    .checked_sub(three_quarter_cycle_time)
                    .unwrap(),
                actual_cycle_start
            ),
        );
        assert_eq!(
            Ok(actual_cycle_start
                .checked_sub(cycle_time - Duration::from_nanos(1))
                .unwrap()),
            skip_missed_cycles(
                cycle_time,
                actual_cycle_start
                    .checked_sub(cycle_time - Duration::from_nanos(1))
                    .unwrap(),
                actual_cycle_start
            ),
        );

        // 1 or more cycles later than expected
        for i in 1u32..10u32 {
            assert_eq!(
                Err((actual_cycle_start, i)),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start.checked_sub(i * cycle_time).unwrap(),
                    actual_cycle_start
                ),
            );
            assert_eq!(
                Err((
                    actual_cycle_start
                        .checked_sub(Duration::from_nanos(1))
                        .unwrap(),
                    i
                )),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start
                        .checked_sub(i * cycle_time)
                        .unwrap()
                        .checked_sub(Duration::from_nanos(1))
                        .unwrap(),
                    actual_cycle_start
                ),
            );
            assert_eq!(
                Err((
                    actual_cycle_start.checked_sub(quarter_cycle_time).unwrap(),
                    i
                )),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start
                        .checked_sub(i * cycle_time)
                        .unwrap()
                        .checked_sub(quarter_cycle_time)
                        .unwrap(),
                    actual_cycle_start
                ),
            );
            assert_eq!(
                Err((actual_cycle_start.checked_sub(half_cycle_time).unwrap(), i)),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start
                        .checked_sub(i * cycle_time)
                        .unwrap()
                        .checked_sub(half_cycle_time)
                        .unwrap(),
                    actual_cycle_start
                ),
            );
            assert_eq!(
                Err((
                    actual_cycle_start
                        .checked_sub(three_quarter_cycle_time)
                        .unwrap(),
                    i
                )),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start
                        .checked_sub(i * cycle_time)
                        .unwrap()
                        .checked_sub(three_quarter_cycle_time)
                        .unwrap(),
                    actual_cycle_start
                ),
            );
            assert_eq!(
                Err((
                    actual_cycle_start
                        .checked_sub(cycle_time - Duration::from_nanos(1))
                        .unwrap(),
                    i
                )),
                skip_missed_cycles(
                    cycle_time,
                    actual_cycle_start
                        .checked_sub(i * cycle_time)
                        .unwrap()
                        .checked_sub(cycle_time - Duration::from_nanos(1))
                        .unwrap(),
                    actual_cycle_start
                ),
            );
        }
    }
}
