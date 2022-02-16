//! Time related types

use std::{
    fmt,
    ops::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign},
    time::{Duration, Instant, SystemTime},
};

use time::{
    error::IndeterminateOffset, format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset,
};

/// A system time with the corresponding instant.
///
/// This should only be used for anchoring values of Instant
/// for conversion into system time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemInstant {
    system_time: SystemTime,
    instant: Instant,
}

impl SystemInstant {
    #[must_use]
    pub const fn new(system_time: SystemTime, instant: Instant) -> Self {
        Self {
            system_time,
            instant,
        }
    }

    #[must_use]
    pub fn now() -> Self {
        let system_time = SystemTime::now();
        // Assumption: The current instant obtained right AFTER receiving
        // the current system time denotes the same point in time and any
        // difference between them is negligible.
        let instant = Instant::now();
        Self::new(system_time, instant)
    }

    #[must_use]
    pub fn system_time(&self) -> SystemTime {
        self.system_time
    }

    #[must_use]
    pub fn instant(&self) -> Instant {
        self.instant
    }

    #[must_use]
    pub fn timestamp_utc(&self) -> Timestamp {
        TimestampInner::from(self.system_time).into()
    }

    #[must_use]
    pub fn checked_duration_since_instant(&self, since_instant: Instant) -> Option<Duration> {
        self.instant.checked_duration_since(since_instant)
    }

    #[must_use]
    pub fn checked_duration_until_instant(&self, until_instant: Instant) -> Option<Duration> {
        until_instant.checked_duration_since(self.instant)
    }

    #[must_use]
    pub fn checked_system_time_for_instant(&self, instant: Instant) -> Option<SystemTime> {
        if self.instant < instant {
            self.system_time
                .checked_add(instant.duration_since(self.instant))
        } else {
            self.system_time
                .checked_sub(self.instant.duration_since(instant))
        }
    }
}

impl Add<Duration> for SystemInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self {
        let Self {
            system_time,
            instant,
        } = self;
        Self::new(system_time + rhs, instant + rhs)
    }
}

impl AddAssign<Duration> for SystemInstant {
    fn add_assign(&mut self, rhs: Duration) {
        let Self {
            mut system_time,
            mut instant,
        } = self;
        system_time += rhs;
        instant += rhs;
    }
}

type TimestampInner = OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(TimestampInner);

impl Timestamp {
    #[must_use]
    pub const fn new(inner: TimestampInner) -> Self {
        Self(inner)
    }

    #[must_use]
    pub const fn to_inner(self) -> TimestampInner {
        let Self(inner) = self;
        inner
    }

    #[must_use]
    pub const fn to_utc(self) -> Self {
        Self(self.to_inner().to_offset(UtcOffset::UTC))
    }

    #[must_use]
    pub fn now() -> Self {
        TimestampInner::now_local()
            .unwrap_or_else(|_: IndeterminateOffset| TimestampInner::now_utc())
            .into()
    }

    #[must_use]
    pub fn now_utc() -> Self {
        TimestampInner::now_utc().into()
    }

    pub fn parse_rfc3339(input: &str) -> Result<Self, time::error::Parse> {
        TimestampInner::parse(input, &Rfc3339).map(Self::new)
    }

    pub fn format_rfc3339(&self) -> Result<String, time::error::Format> {
        self.0.format(&Rfc3339)
    }

    pub fn format_rfc3339_into<W: std::io::Write>(
        &self,
        output: &mut W,
    ) -> Result<usize, time::error::Format> {
        self.0.format_into(output, &Rfc3339)
    }
}

impl From<TimestampInner> for Timestamp {
    fn from(inner: TimestampInner) -> Self {
        Self::new(inner)
    }
}

impl From<Timestamp> for TimestampInner {
    fn from(from: Timestamp) -> Self {
        from.to_inner()
    }
}

impl From<SystemTime> for Timestamp {
    fn from(system_time: SystemTime) -> Self {
        Self::new(system_time.into())
    }
}

impl From<Timestamp> for SystemTime {
    fn from(from: Timestamp) -> Self {
        from.to_inner().into()
    }
}

impl AsRef<TimestampInner> for Timestamp {
    fn as_ref(&self) -> &TimestampInner {
        &self.0
    }
}

impl Deref for Timestamp {
    type Target = TimestampInner;

    fn deref(&self) -> &TimestampInner {
        self.as_ref()
    }
}

impl DerefMut for Timestamp {
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        let Self(inner) = self;
        inner
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

#[cfg(feature = "with-serde")]
impl serde::Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        time::serde::rfc3339::serialize(self.as_ref(), serializer)
    }
}

#[cfg(feature = "with-serde")]
impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        time::serde::rfc3339::deserialize(deserializer).map(Self::new)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Nanos(u32),
    Micros(u32),
    Millis(u32),
    Seconds(u32),
    Minutes(u32),
    Hours(u32),
    Days(u32),
    Weeks(u32),
}

impl Interval {
    fn as_duration(self) -> Duration {
        match self {
            Self::Nanos(nanos) => Duration::from_nanos(u64::from(nanos)),
            Self::Micros(micros) => Duration::from_micros(u64::from(micros)),
            Self::Millis(millis) => Duration::from_millis(u64::from(millis)),
            Self::Seconds(secs) => Duration::from_secs(u64::from(secs)),
            Self::Minutes(mins) => Duration::from_secs(u64::from(mins) * 60),
            Self::Hours(hrs) => Duration::from_secs(u64::from(hrs) * 60 * 60),
            Self::Days(days) => Duration::from_secs(u64::from(days) * 60 * 60 * 24),
            Self::Weeks(weeks) => Duration::from_secs(u64::from(weeks) * 60 * 60 * 24 * 7),
        }
    }

    #[must_use]
    pub fn system_time_before(&self, system_time: SystemTime) -> SystemTime {
        system_time - self.as_duration()
    }

    #[must_use]
    pub fn system_time_after(&self, system_time: SystemTime) -> SystemTime {
        system_time + self.as_duration()
    }
}

impl Add<Interval> for Timestamp {
    type Output = Timestamp;

    fn add(self, interval: Interval) -> Self::Output {
        (self.to_inner() + interval.as_duration()).into()
    }
}

impl AddAssign<Interval> for Timestamp {
    fn add_assign(&mut self, interval: Interval) {
        *self = *self + interval;
    }
}

impl Sub<Interval> for Timestamp {
    type Output = Timestamp;

    fn sub(self, interval: Interval) -> Self::Output {
        (self.to_inner() - interval.as_duration()).into()
    }
}

impl SubAssign<Interval> for Timestamp {
    fn sub_assign(&mut self, interval: Interval) {
        *self = *self - interval;
    }
}
