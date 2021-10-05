//! Journaling features

use std::{
    convert::{TryFrom, TryInto},
    num::NonZeroUsize,
    path::PathBuf,
    time::SystemTime,
};

use ::csv::Reader as CsvReader;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    io::file::{
        policy::{RollingFileInfo, RollingFileNameTemplate},
        WriteResult,
    },
    storage::{
        self, csv, CreatedAtOffset, CreatedAtOffsetNanos, ReadableRecordPrelude,
        RecordPreludeFilter, RecordStorageBase, RecordStorageRead, RecordStorageWrite,
        StorageConfig, StorageDescriptor, StorageStatistics, WritableRecordPrelude,
        MAX_PREALLOCATED_CAPACITY_LIMIT,
    },
    time::SystemTimeInstant,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] storage::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<std::io::Error> for Error {
    fn from(from: std::io::Error) -> Self {
        Self::Storage(storage::Error::Io(from))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub type SeverityValue = u8;

#[derive(Debug)]
pub struct SeverityValues;

impl SeverityValues {
    pub const DIAGNOSTIC_VERBOSE: SeverityValue = 1;
    pub const DIAGNOSTIC: SeverityValue = 2;
    pub const INFORMATION_VERBOSE: SeverityValue = 3;
    pub const INFORMATION: SeverityValue = 4;
    pub const WARNING: SeverityValue = 5;
    pub const WARNING_UNEXPECTED: SeverityValue = 6;
    pub const ERROR: SeverityValue = 7;
    pub const ERROR_CRITICAL: SeverityValue = 8;
}

/// A measure for the significance and/or priority of an entry.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    DiagnosticVerbose = SeverityValues::DIAGNOSTIC_VERBOSE as isize,

    Diagnostic = SeverityValues::DIAGNOSTIC as isize,

    InformationVerbose = SeverityValues::INFORMATION_VERBOSE as isize,

    Information = SeverityValues::INFORMATION as isize,

    Warning = SeverityValues::WARNING as isize,

    WarningUnexpected = SeverityValues::WARNING_UNEXPECTED as isize,

    Error = SeverityValues::ERROR as isize,

    ErrorCritical = SeverityValues::ERROR_CRITICAL as isize,
}

impl Severity {
    pub fn is_diagnostic(self) -> bool {
        self == Self::Diagnostic || self == Self::InformationVerbose
    }

    pub fn is_information(self) -> bool {
        self == Self::Information || self == Self::InformationVerbose
    }

    pub fn is_warning(self) -> bool {
        self == Self::Warning || self == Self::WarningUnexpected
    }

    pub fn is_error(self) -> bool {
        self == Self::Error || self == Self::ErrorCritical
    }

    pub const fn value(self) -> SeverityValue {
        self as SeverityValue
    }
}

impl From<Severity> for SeverityValue {
    fn from(from: Severity) -> Self {
        from.value()
    }
}

#[derive(Error, Debug)]
pub enum TryFromSeverityValueError {
    #[error("invalid value {0}")]
    InvalidValue(SeverityValue),
}

impl TryFrom<SeverityValue> for Severity {
    type Error = TryFromSeverityValueError;

    fn try_from(from: SeverityValue) -> std::result::Result<Self, TryFromSeverityValueError> {
        match from {
            SeverityValues::DIAGNOSTIC_VERBOSE => Ok(Severity::DiagnosticVerbose),
            SeverityValues::DIAGNOSTIC => Ok(Severity::Diagnostic),
            SeverityValues::INFORMATION_VERBOSE => Ok(Severity::InformationVerbose),
            SeverityValues::INFORMATION => Ok(Severity::Information),
            SeverityValues::WARNING => Ok(Severity::Warning),
            SeverityValues::WARNING_UNEXPECTED => Ok(Severity::WarningUnexpected),
            SeverityValues::ERROR => Ok(Severity::Error),
            SeverityValues::ERROR_CRITICAL => Ok(Severity::ErrorCritical),
            _ => Err(TryFromSeverityValueError::InvalidValue(from)),
        }
    }
}

pub type ScopeValue = String;

/// Symbolic scope name
///
/// A technical identifier for the origin or source of the
/// event. It uniquely identifies the system component and
/// the context within this component that caused the event.
///
/// The number of possible values should be restricted to
/// limited, predefined set. Those values usually depend on
/// the system configuration and may follow some naming
/// conventions that could be parsed.
// Symbolic name that identifies the scope of a journal entry.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Scope(pub String);

impl From<ScopeValue> for Scope {
    fn from(inner: ScopeValue) -> Self {
        Self(inner)
    }
}

impl From<Scope> for ScopeValue {
    fn from(from: Scope) -> Self {
        let Scope(inner) = from;
        inner
    }
}

pub type CodeValue = i32;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Code(pub CodeValue);

impl From<CodeValue> for Code {
    fn from(inner: CodeValue) -> Self {
        Self(inner)
    }
}

impl From<Code> for CodeValue {
    fn from(from: Code) -> Self {
        let Code(inner) = from;
        inner
    }
}

/// A journal entry
///
/// Stores information about events or incidents that happened in the system.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Entry {
    pub occurred_at: SystemTime,

    pub severity: Severity,

    /// Identifies context: component -> sub-component -> use case -> function -> ...
    pub scope: Scope,

    /// Scope-dependent code
    pub code: Code,

    /// Textual context description
    pub text: Option<String>,

    /// Serialized JSON context data
    pub json: Option<String>,
}

pub type RecordIdType = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordId(pub RecordIdType);

impl From<RecordIdType> for RecordId {
    fn from(inner: RecordIdType) -> Self {
        Self(inner)
    }
}

impl From<RecordId> for RecordIdType {
    fn from(from: RecordId) -> Self {
        let RecordId(inner) = from;
        inner
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordPrelude {
    pub id: RecordId,

    pub created_at_offset: CreatedAtOffset,
}

impl WritableRecordPrelude for RecordPrelude {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        debug_assert_eq!(self.created_at_offset, Default::default()); // not yet initialized
        self.created_at_offset = created_at_offset;
    }
}

/// A recorded journal entry
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Record {
    pub prelude: RecordPrelude,

    pub entry: Entry,
}

impl ReadableRecordPrelude for Record {
    fn created_at_offset(&self) -> CreatedAtOffset {
        self.prelude.created_at_offset
    }
}

impl WritableRecordPrelude for Record {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        self.prelude.set_created_at_offset(created_at_offset)
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct RecordFilter {
    pub prelude: RecordPreludeFilter,
    pub min_severity: Option<Severity>,
    pub any_scopes: Option<Vec<Scope>>,
    pub any_codes: Option<Vec<Code>>,
}

pub trait RecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)>;
}

#[derive(Debug)]
pub struct DefaultRecordPreludeGenerator;

impl RecordPreludeGenerator for DefaultRecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)> {
        let id = RecordId::from(bs58::encode(Uuid::new_v4().as_bytes()).into_string());
        Ok((
            SystemTimeInstant::now(),
            RecordPrelude {
                id,
                created_at_offset: Default::default(),
            },
        ))
    }
}

pub trait RecordStorage: RecordStorageBase + RecordStorageWrite<Record> {
    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<StoredRecord>>;

    fn filter_records(
        &mut self,
        limit: NonZeroUsize,
        filter: RecordFilter,
    ) -> Result<Vec<StoredRecord>>;
}

// Fields ordered according to filtering and access patterns, i.e. most
// frequently used fields first.
#[derive(Debug, Serialize, Deserialize)]
struct StorageRecord {
    created_at_offset_ns: CreatedAtOffsetNanos,

    occurred_at: DateTime<Utc>,

    severity: SeverityValue,

    scope: String,

    code: CodeValue,

    id: String,

    text: Option<String>,

    json: Option<String>,
}

impl ReadableRecordPrelude for StorageRecord {
    fn created_at_offset(&self) -> CreatedAtOffset {
        self.created_at_offset_ns.into()
    }
}

impl WritableRecordPrelude for StorageRecord {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        self.created_at_offset_ns = created_at_offset.into();
    }
}

impl From<Record> for StorageRecord {
    fn from(from: Record) -> Self {
        let Record {
            prelude:
                RecordPrelude {
                    id,
                    created_at_offset,
                },
            entry:
                Entry {
                    occurred_at,
                    severity,
                    scope,
                    code,
                    text,
                    json,
                },
        } = from;
        Self {
            created_at_offset_ns: created_at_offset.into(),
            occurred_at: DateTime::from(occurred_at),
            severity: SeverityValue::from(severity),
            scope: scope.0,
            code: code.0,
            id: id.0,
            text,
            json,
        }
    }
}

/// A stored journal entry
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StoredRecordPrelude {
    pub id: RecordId,

    pub created_at: SystemTime,
}

/// A stored journal entry
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StoredRecord {
    pub prelude: StoredRecordPrelude,

    pub entry: Entry,
}

impl StoredRecord {
    fn try_restore(created_at_origin: SystemTime, record: StorageRecord) -> Result<Self> {
        let StorageRecord {
            created_at_offset_ns,
            occurred_at,
            severity,
            scope,
            code,
            id,
            text,
            json,
        } = record;
        let created_at = created_at_origin + CreatedAtOffset::from(created_at_offset_ns).into();
        let prelude = StoredRecordPrelude {
            id: id.into(),
            created_at,
        };
        Ok(Self {
            prelude,
            entry: Entry {
                occurred_at: occurred_at.into(),
                severity: severity.try_into().map_err(anyhow::Error::from)?,
                scope: scope.into(),
                code: code.into(),
                text,
                json,
            },
        })
    }
}

#[allow(missing_debug_implementations)]
pub struct CsvFileRecordStorage {
    inner: csv::FileRecordStorage<StorageRecord, StorageRecord>,
}

impl CsvFileRecordStorage {
    pub fn try_new(config: StorageConfig, base_path: PathBuf) -> Result<Self> {
        let file_name_template = RollingFileNameTemplate {
            prefix: "journal_".to_string(),
            suffix: ".csv".to_string(),
        };
        let inner = csv::FileRecordStorage::try_new(config, base_path, file_name_template, None)?;
        Ok(Self { inner })
    }
}

fn filter_map_storage_record(
    created_at_origin: SystemTime,
    record: StorageRecord,
) -> Option<StoredRecord> {
    match StoredRecord::try_restore(created_at_origin, record) {
        Ok(record) => Some(record),
        Err(err) => {
            // This should never happen
            log::error!("Failed to convert record: {}", err);
            // Skip and continue
            None
        }
    }
}

impl RecordStorageBase for CsvFileRecordStorage {
    fn descriptor(&self) -> &StorageDescriptor {
        self.inner.descriptor()
    }

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig {
        self.inner.replace_config(new_config)
    }

    fn perform_housekeeping(&mut self) -> storage::Result<()> {
        self.inner.perform_housekeeping()
    }

    fn retain_all_records_created_since(
        &mut self,
        created_since: SystemTime,
    ) -> storage::Result<()> {
        self.inner.retain_all_records_created_since(created_since)
    }

    fn report_statistics(&mut self) -> storage::Result<StorageStatistics> {
        self.inner.report_statistics()
    }
}

impl RecordStorageWrite<Record> for CsvFileRecordStorage {
    fn append_record(
        &mut self,
        created_at: &SystemTimeInstant,
        record: Record,
    ) -> storage::Result<(WriteResult, CreatedAtOffset)> {
        self.inner
            .append_record(created_at, StorageRecord::from(record))
    }
}

impl RecordStorage for CsvFileRecordStorage {
    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<StoredRecord>> {
        // TODO: How to avoid conversion of (intermediate) vectors?
        self.inner
            .recent_records(limit)
            .map(|v| {
                v.into_iter()
                    // TODO: filter_map() may drop some inconvertible records that have
                    // not been accounted for when prematurely limiting the results
                    // requested from self.inner (see above)! This should not happen
                    // and an error is logged.
                    .filter_map(|(create_at_origin, record)| {
                        filter_map_storage_record(create_at_origin, record)
                    })
                    .collect()
            })
            .map_err(Error::Storage)
    }

    fn filter_records(
        &mut self,
        limit: NonZeroUsize,
        filter: RecordFilter,
    ) -> Result<Vec<StoredRecord>> {
        self.inner.flush_before_reading()?;
        let limit = limit.get().min(MAX_PREALLOCATED_CAPACITY_LIMIT);
        let mut records = Vec::with_capacity(limit);
        for file_info in self
            .inner
            .read_all_dir_entries_filtered_chronologically(
                &csv::file_info_filter_from_record_prelude_filter(filter.prelude.clone()),
            )?
            .into_iter()
            .map(RollingFileInfo::from)
        {
            if limit <= records.len() {
                break;
            }
            let remaining_limit = limit - records.len();
            let reader = csv::create_file_reader(&file_info.path)?;
            records = csv_reader_into_filtered_record_iter(
                reader,
                file_info.created_at.into(),
                filter.clone(),
            )
            .take(remaining_limit)
            .fold(records, |mut records, entry| {
                records.push(entry);
                records
            });
        }
        Ok(records)
    }
}

fn csv_reader_into_filtered_record_iter<R>(
    reader: CsvReader<R>,
    created_at_origin: SystemTime,
    filter: RecordFilter,
) -> impl Iterator<Item = StoredRecord>
where
    R: std::io::Read,
{
    let RecordFilter {
        prelude: prelude_filter,
        any_codes,
        any_scopes,
        min_severity,
    } = filter;
    csv::reader_into_filtered_record_iter(reader, created_at_origin, prelude_filter)
        .filter_map(move |record| filter_map_storage_record(created_at_origin, record))
        .filter(move |StoredRecord { prelude: _, entry }| {
            if let Some(min_severity) = min_severity {
                if entry.severity < min_severity {
                    return false;
                }
            }
            if let Some(any_codes) = &any_codes {
                if any_codes.iter().all(|code| *code != entry.code) {
                    return false;
                }
            }
            if let Some(any_scopes) = &any_scopes {
                if any_scopes.iter().all(|scope| scope != &entry.scope) {
                    return false;
                }
            }
            true
        })
}
