//! Persistence related stuff

use std::{
    collections::VecDeque,
    io::Error as IoError,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    result::Result as StdResult,
    time::{Duration, SystemTime},
};

use thiserror::Error;

use crate::{
    fs::WriteResult,
    time::{Interval, SystemInstant},
};

// TODO: Currently unused
pub mod field;

#[cfg(feature = "with-csv-storage")]
pub mod csv;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] IoError),

    #[cfg(feature = "with-csv-storage")]
    #[error(transparent)]
    Csv(#[from] ::csv::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(feature = "with-csv-storage")]
impl From<crate::fs::csv::Error> for Error {
    fn from(err: crate::fs::csv::Error) -> Self {
        use crate::fs::csv::Error::*;
        match err {
            Io(err) => Error::Io(err),
            Csv(err) => Error::Csv(err),
        }
    }
}

pub type Result<T> = StdResult<T, Error>;

// Maximum pre-allocated capacity to avoid allocation errors
// caused by excessively high capacity or limit parameters
pub const MAX_PREALLOCATED_CAPACITY_LIMIT: usize = 16_384; // 2^14

#[derive(Debug, Clone)]
pub struct StorageStatus {
    pub descriptor: StorageDescriptor,
    pub statistics: Option<StorageStatistics>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TimeInterval {
    Days(NonZeroU32),
}

const SECONDS_PER_DAY: u64 = 24 * 3_600;

impl From<TimeInterval> for Duration {
    fn from(from: TimeInterval) -> Self {
        use TimeInterval::*;
        match from {
            Days(days) => Duration::from_secs(SECONDS_PER_DAY * u64::from(days.get())),
        }
    }
}

impl From<TimeInterval> for Interval {
    fn from(from: TimeInterval) -> Self {
        use TimeInterval::*;
        match from {
            Days(days) => Interval::Days(days.get()),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MemorySize {
    Bytes(NonZeroU64),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StorageConfig {
    pub retention_time: TimeInterval,
    pub segmentation: StorageSegmentConfig,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StorageSegmentConfig {
    pub time_interval: TimeInterval,
    pub size_limit: MemorySize,
}

#[derive(Debug, Clone)]
pub struct StorageDescriptor {
    pub kind: String,
    pub base_path: Option<PathBuf>,
    pub binary_data_format: BinaryDataFormat,
}

#[derive(Debug, Clone)]
pub struct StorageStatistics {
    /// The total number of records (if known)
    pub total_records: Option<usize>,

    /// The total size in bytes (if known)
    pub total_bytes: Option<u64>,

    /// Segment statistics (if applicable and available)
    pub segments: Option<Vec<StorageSegmentStatistics>>,
}

#[derive(Debug, Clone)]
pub struct StorageSegmentStatistics {
    pub created_at: SystemTime,

    pub total_records: usize,

    /// The total size in bytes (if known)
    pub total_bytes: Option<u64>,
}

pub trait ReadableRecordPrelude {
    fn created_at_offset(&self) -> CreatedAtOffset;
}

pub trait WritableRecordPrelude {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset);
}

pub type CreatedAtOffsetNanos = u64;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct CreatedAtOffset {
    nanos: CreatedAtOffsetNanos,
}

impl CreatedAtOffset {
    #[must_use]
    pub fn system_time_from_origin(self, origin: SystemTime) -> SystemTime {
        origin + Duration::from(self)
    }

    #[must_use]
    pub const fn to_duration(self) -> Duration {
        let Self { nanos } = self;
        Duration::from_nanos(nanos)
    }
}

impl From<CreatedAtOffsetNanos> for CreatedAtOffset {
    fn from(nanos: CreatedAtOffsetNanos) -> Self {
        Self { nanos }
    }
}

impl From<CreatedAtOffset> for CreatedAtOffsetNanos {
    fn from(from: CreatedAtOffset) -> Self {
        let CreatedAtOffset { nanos } = from;
        nanos
    }
}

impl From<Duration> for CreatedAtOffset {
    fn from(from: Duration) -> Self {
        let nanos = from.as_nanos();
        // TODO: Handle overflow?
        debug_assert!(nanos <= u128::from(CreatedAtOffsetNanos::MAX));
        Self {
            nanos: nanos as CreatedAtOffsetNanos,
        }
    }
}

impl From<CreatedAtOffset> for Duration {
    fn from(from: CreatedAtOffset) -> Self {
        from.to_duration()
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct RecordPreludeFilter {
    pub since_created_at: Option<SystemTime>,
    pub until_created_at: Option<SystemTime>,
}

pub trait RecordStorageBase {
    fn descriptor(&self) -> &StorageDescriptor;

    fn config(&self) -> &StorageConfig;

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig;

    fn perform_housekeeping(&mut self) -> Result<()>;

    /// Try to drop records that have been created before the given time
    fn retain_all_records_created_since(&mut self, created_since: SystemTime) -> Result<()>;

    fn report_statistics(&mut self) -> Result<StorageStatistics>;
}

/// Format of custom, binary data
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BinaryDataFormat {
    /// Arbitrary binary data
    ///
    /// Serialized as Base64 with standard alphabet and no padding.
    Bytes,

    /// Serialized UTF-8 data
    ///
    /// A typical use case is the tunneling of UTF-8 JSON data.
    Utf8,
}

impl Default for BinaryDataFormat {
    fn default() -> Self {
        Self::Bytes
    }
}

fn encode_binary_data_bytes(input: impl AsRef<[u8]>) -> String {
    base64::encode_config(&input, base64::STANDARD_NO_PAD)
}

fn encode_binary_data_utf8(input: Vec<u8>) -> anyhow::Result<String> {
    String::from_utf8(input).map_err(Into::into)
}

pub fn encode_binary_data_into_string(
    input: Vec<u8>,
    format: BinaryDataFormat,
) -> anyhow::Result<String> {
    match format {
        BinaryDataFormat::Bytes => Ok(encode_binary_data_bytes(&input)),
        BinaryDataFormat::Utf8 => encode_binary_data_utf8(input),
    }
}

fn decode_binary_data_bytes(input: impl AsRef<[u8]>) -> anyhow::Result<Vec<u8>> {
    base64::decode_config(input, base64::STANDARD_NO_PAD).map_err(anyhow::Error::from)
}

fn decode_binary_data_utf8(input: String) -> Vec<u8> {
    input.into_bytes()
}

pub fn decode_binary_data_from_string(
    input: String,
    format: BinaryDataFormat,
) -> anyhow::Result<Vec<u8>> {
    match format {
        BinaryDataFormat::Bytes => decode_binary_data_bytes(&input),
        BinaryDataFormat::Utf8 => Ok(decode_binary_data_utf8(input)),
    }
}

pub trait RecordStorageWrite<R>: RecordStorageBase
where
    R: WritableRecordPrelude,
{
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
        record: R,
    ) -> Result<(WriteResult, CreatedAtOffset)>;
}

pub trait RecordStorageRead<R>: RecordStorageBase {
    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<(SystemTime, R)>>;
}

#[allow(missing_debug_implementations)]
pub struct InMemoryRecordStorage<R> {
    config: StorageConfig,
    descriptor: StorageDescriptor,
    created_at_origin: SystemInstant,
    records: VecDeque<R>,
    _record_phantom: std::marker::PhantomData<R>,
}

impl<R> InMemoryRecordStorage<R>
where
    R: Clone,
{
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        let descriptor = StorageDescriptor {
            kind: "in-memory".to_string(),
            base_path: None,
            binary_data_format: Default::default(), // no serialization
        };
        Self {
            config,
            descriptor,
            created_at_origin: SystemInstant::now(),
            records: VecDeque::with_capacity(MAX_PREALLOCATED_CAPACITY_LIMIT),
            _record_phantom: Default::default(),
        }
    }

    pub fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<R>> {
        let total_count = self.records.len();
        let limited_count = limit.get().min(total_count);
        Ok(self
            .records
            .iter()
            .skip(total_count - limited_count)
            .cloned()
            .collect())
    }
}

impl<R> RecordStorageBase for InMemoryRecordStorage<R>
where
    R: ReadableRecordPrelude,
{
    fn descriptor(&self) -> &StorageDescriptor {
        &self.descriptor
    }

    fn config(&self) -> &StorageConfig {
        &self.config
    }

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig {
        std::mem::replace(&mut self.config, new_config)
    }

    fn perform_housekeeping(&mut self) -> Result<()> {
        Ok(())
    }

    fn retain_all_records_created_since(&mut self, created_since: SystemTime) -> Result<()> {
        let created_since_offset = created_since
            .duration_since(self.created_at_origin.system_time())
            .unwrap_or_default()
            .into();
        while let Some(first) = self.records.front() {
            if first.created_at_offset() >= created_since_offset {
                break;
            }
            self.records.pop_front();
        }
        Ok(())
    }

    fn report_statistics(&mut self) -> Result<StorageStatistics> {
        Ok(StorageStatistics {
            total_records: Some(self.records.len()),
            total_bytes: None,
            segments: None,
        })
    }
}

impl<R> RecordStorageWrite<R> for InMemoryRecordStorage<R>
where
    R: ReadableRecordPrelude + WritableRecordPrelude,
{
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
        mut record: R,
    ) -> Result<(WriteResult, CreatedAtOffset)> {
        debug_assert!(created_at.instant() >= self.created_at_origin.instant());
        let created_at_offset =
            CreatedAtOffset::from(created_at.instant() - self.created_at_origin.instant());
        debug_assert_eq!(record.created_at_offset(), Default::default()); // not yet initialized
        record.set_created_at_offset(created_at_offset);
        self.records.push_back(record);
        Ok((Ok(()), created_at_offset))
    }
}
