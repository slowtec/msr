//! Time related structs
use std::{
    fmt::Debug,
    ops::{Add, AddAssign},
    time::{Duration, Instant, SystemTime},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SystemTimeInstantError {
    #[error("instant occurred in the past")]
    SoonerInstant,
}

/// A system time the corresponding instant.
///
/// This should only be used for anchoring values of Instant
/// for conversion into system time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemTimeInstant {
    system_time: SystemTime,
    instant: Instant,
}

impl SystemTimeInstant {
    pub const fn new(system_time: SystemTime, instant: Instant) -> Self {
        Self {
            system_time,
            instant,
        }
    }

    pub fn now() -> Self {
        let instant = Instant::now();
        let system_time = SystemTime::now();
        let elapsed = instant.elapsed();
        // Compensate for the time that passed while retrieving
        // the current system time
        let instant = instant + elapsed / 2;
        Self::new(system_time, instant)
    }

    pub fn system_time(&self) -> SystemTime {
        self.system_time
    }

    pub fn instant(&self) -> Instant {
        self.instant
    }

    pub fn duration_until_instant(
        &self,
        until_instant: Instant,
    ) -> Result<Duration, SystemTimeInstantError> {
        if self.instant <= until_instant {
            Ok(until_instant - self.instant)
        } else {
            Err(SystemTimeInstantError::SoonerInstant)
        }
    }

    pub fn map_instant_to_system_time(
        &self,
        later_instant: Instant,
    ) -> Result<SystemTime, SystemTimeInstantError> {
        Ok(self.system_time + self.duration_until_instant(later_instant)?)
    }
}

impl Add<Duration> for SystemTimeInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self {
        let Self {
            system_time,
            instant,
        } = self;
        Self::new(system_time + rhs, instant + rhs)
    }
}

impl AddAssign<Duration> for SystemTimeInstant {
    fn add_assign(&mut self, rhs: Duration) {
        let Self {
            mut system_time,
            mut instant,
        } = self;
        system_time += rhs;
        instant += rhs;
    }
}
