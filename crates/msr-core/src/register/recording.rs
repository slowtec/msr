use std::{
    io, iter,
    num::{NonZeroUsize, ParseIntError},
    path::PathBuf,
    result::Result as StdResult,
    time::SystemTime,
};

use ::csv::StringRecord as CsvStringRecord;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    io::file::policy::RollingFileNameTemplate,
    register,
    storage::{
        self, csv, CreatedAtOffset, CreatedAtOffsetNanos, ReadableRecordPrelude,
        RecordPreludeFilter, RecordStorageBase, RecordStorageRead, RecordStorageWrite,
        StorageConfig, StorageDescriptor, StorageStatistics, WritableRecordPrelude,
    },
    time::SystemTimeInstant,
    ScalarType, ScalarValue, ToValueType, Value, ValueType,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("mismatching register types: expected = {expected:?}, actual = {actual:?}")]
    MismatchingRegisterTypes {
        expected: ValueType,
        actual: ValueType,
    },

    #[error(transparent)]
    Storage(#[from] storage::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = StdResult<T, Error>;

/// An observation of register values
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedRegisterValues<RegisterValue> {
    pub observed_at: SystemTime,

    pub register_values: Vec<Option<RegisterValue>>,
}

impl<RegisterValue> From<register::ObservedValues<RegisterValue>>
    for ObservedRegisterValues<RegisterValue>
{
    fn from(from: register::ObservedValues<RegisterValue>) -> Self {
        let register::ObservedValues {
            observed_at,
            values,
        } = from;
        Self {
            // Drop the Instant part
            observed_at: observed_at.system_time(),
            register_values: values,
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct RecordPrelude {
    pub created_at_offset: CreatedAtOffset,
}

impl WritableRecordPrelude for RecordPrelude {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        debug_assert_eq!(self.created_at_offset, Default::default()); // not yet initialized
        self.created_at_offset = created_at_offset;
    }
}

/// An observation of register values
#[derive(Debug, Clone, PartialEq)]
pub struct Record<Value> {
    pub prelude: RecordPrelude,

    pub observation: ObservedRegisterValues<Value>,
}

impl<Value> WritableRecordPrelude for Record<Value> {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        self.prelude.set_created_at_offset(created_at_offset)
    }
}

pub trait RecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)>;
}

#[derive(Debug)]
pub struct DefaultRecordPreludeGenerator;

impl RecordPreludeGenerator for DefaultRecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemTimeInstant, RecordPrelude)> {
        Ok((SystemTimeInstant::now(), Default::default()))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StoredRecordPrelude {
    pub created_at: SystemTime,
}

impl StoredRecordPrelude {
    fn create(created_at: SystemTime) -> Self {
        Self { created_at }
    }

    fn restore(created_at_origin: SystemTime, prelude: RecordPrelude) -> Self {
        let created_at = created_at_origin + prelude.created_at_offset.into();
        Self { created_at }
    }
}

pub trait RecordStorage<RegisterValue>: RecordStorageBase {
    fn append_record(
        &mut self,
        created_at: &SystemTimeInstant,
        record: Record<RegisterValue>,
    ) -> Result<StoredRecordPrelude>;

    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<StoredRecord<RegisterValue>>>;

    fn filter_records(
        &mut self,
        limit: NonZeroUsize,
        filter: &RecordPreludeFilter,
    ) -> Result<Vec<StoredRecord<RegisterValue>>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerdeRegisterValue {
    /// Boolean
    Bool(bool),
    /// 64-bit signed integer
    I64(i64),
    /// 64-bit unsigned integer
    U64(u64),
    /// 64-bit floating-point number (double precision)
    F64(f64),
    /// A string
    String(String),
}

#[test]
fn serialize_scalar_value() {
    assert_eq!(
        serde_json::to_string(&SerdeRegisterValue::U64(5)).unwrap(),
        "5"
    );
    assert_eq!(
        serde_json::to_string(&SerdeRegisterValue::Bool(true)).unwrap(),
        "true"
    );
}

#[test]
fn deserialize_scalar_value() {
    assert_eq!(
        serde_json::from_str::<SerdeRegisterValue>("5").unwrap(),
        SerdeRegisterValue::I64(5)
    );
    assert_eq!(
        serde_json::from_str::<SerdeRegisterValue>("5.0").unwrap(),
        SerdeRegisterValue::F64(5.0)
    );
    assert_eq!(
        serde_json::from_str::<SerdeRegisterValue>("true").unwrap(),
        SerdeRegisterValue::Bool(true)
    );
}

impl From<ScalarValue> for SerdeRegisterValue {
    fn from(from: ScalarValue) -> Self {
        use ScalarValue as S;
        match from {
            S::Bool(val) => Self::Bool(val),
            S::I8(val) => Self::I64(i64::from(val)),
            S::U8(val) => Self::U64(u64::from(val)),
            S::I16(val) => Self::I64(i64::from(val)),
            S::U16(val) => Self::U64(u64::from(val)),
            S::I32(val) => Self::I64(i64::from(val)),
            S::U32(val) => Self::U64(u64::from(val)),
            S::F32(val) => Self::F64(f64::from(val)),
            S::I64(val) => Self::I64(val),
            S::U64(val) => Self::U64(val),
            S::F64(val) => Self::F64(val),
        }
    }
}

impl From<Value> for SerdeRegisterValue {
    fn from(from: Value) -> Self {
        use Value as V;
        match from {
            V::Scalar(val) => Self::from(val),
            V::String(val) => Self::String(val),
            V::Duration(_) => unimplemented!(),
            V::Bytes(_) => unimplemented!(),
        }
    }
}

impl From<SerdeRegisterValue> for crate::Value {
    fn from(from: SerdeRegisterValue) -> Self {
        use ScalarValue as S;
        use SerdeRegisterValue::*;
        match from {
            Bool(val) => Self::Scalar(S::Bool(val)),
            I64(val) => Self::Scalar(S::I64(val)),
            U64(val) => Self::Scalar(S::U64(val)),
            F64(val) => Self::Scalar(S::F64(val)),
            String(val) => Self::String(val),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredRecord<RegisterValue> {
    pub prelude: StoredRecordPrelude,

    pub observation: ObservedRegisterValues<RegisterValue>,
}

impl<RegisterValue> StoredRecord<RegisterValue> {
    fn restore(created_at_origin: SystemTime, record: Record<RegisterValue>) -> Self {
        let Record {
            prelude,
            observation,
        } = record;
        let prelude = StoredRecordPrelude::restore(created_at_origin, prelude);
        Self {
            prelude,
            observation,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StorageRecord {
    created_at_offset_ns: CreatedAtOffsetNanos,

    observed_at: DateTime<Utc>,

    register_values: Vec<Option<SerdeRegisterValue>>,
}

impl ReadableRecordPrelude for StorageRecord {
    fn created_at_offset(&self) -> CreatedAtOffset {
        self.created_at_offset_ns.into()
    }
}

impl WritableRecordPrelude for StorageRecord {
    fn set_created_at_offset(&mut self, created_at_offset: CreatedAtOffset) {
        debug_assert_eq!(
            CreatedAtOffset::from(self.created_at_offset_ns),
            Default::default()
        ); // not yet initialized
        self.created_at_offset_ns = created_at_offset.into();
    }
}

struct StorageRecordDeserializer {
    registers: Vec<(register::Index, ValueType)>,
}

#[derive(thiserror::Error, Debug)]
pub enum StorageRecordDeserializeError {
    #[error("mismatching number of fields: expected = {expected:?}, actual = {actual:?}")]
    MismatchingNumberOfFields { expected: usize, actual: usize },

    #[error(transparent)]
    ParseCreatedAtOffset(ParseIntError),

    #[error(transparent)]
    ParseObservedAt(chrono::ParseError),

    #[error(transparent)]
    ParseRegisterValue(anyhow::Error),
}

impl csv::StringRecordDeserializer<StorageRecord> for StorageRecordDeserializer {
    fn deserialize_string_record(
        &self,
        record: &CsvStringRecord,
    ) -> storage::Result<StorageRecord> {
        if record.len() != 2 + self.registers.len() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                StorageRecordDeserializeError::MismatchingNumberOfFields {
                    expected: 2 + self.registers.len(),
                    actual: record.len(),
                },
            )
            .into());
        }
        let mut record_fields = record.iter();
        let created_at_offset_ns = record_fields
            .next()
            .unwrap()
            .parse::<CreatedAtOffsetNanos>()
            .map_err(StorageRecordDeserializeError::ParseCreatedAtOffset)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let observed_at = DateTime::parse_from_rfc3339(record_fields.next().unwrap())
            .map_err(StorageRecordDeserializeError::ParseObservedAt)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
            .into();
        let mut register_values = Vec::with_capacity(self.registers.len());
        for (record_field, (register_index, register_type)) in record_fields.zip(&self.registers) {
            if record_field.is_empty() {
                register_values.push(None);
                continue;
            }
            let parsed_record_field = match register_type {
                ValueType::Scalar(t) => match t {
                    ScalarType::Bool => record_field
                        .parse::<bool>()
                        .map(SerdeRegisterValue::Bool)
                        .map_err(|err| err.to_string()),
                    ScalarType::I16 => record_field
                        .parse::<i64>()
                        .map(SerdeRegisterValue::I64)
                        .map_err(|err| err.to_string()),
                    ScalarType::U16 => record_field
                        .parse::<u64>()
                        .map(SerdeRegisterValue::U64)
                        .map_err(|err| err.to_string()),
                    ScalarType::I32 => record_field
                        .parse::<i64>()
                        .map(SerdeRegisterValue::I64)
                        .map_err(|err| err.to_string()),
                    ScalarType::U32 => record_field
                        .parse::<u64>()
                        .map(SerdeRegisterValue::U64)
                        .map_err(|err| err.to_string()),
                    ScalarType::F32 => record_field
                        .parse::<f64>()
                        .map(SerdeRegisterValue::F64)
                        .map_err(|err| err.to_string()),
                    ScalarType::I64 => record_field
                        .parse::<i64>()
                        .map(SerdeRegisterValue::I64)
                        .map_err(|err| err.to_string()),
                    ScalarType::U64 => record_field
                        .parse::<u64>()
                        .map(SerdeRegisterValue::U64)
                        .map_err(|err| err.to_string()),
                    ScalarType::F64 => record_field
                        .parse::<f64>()
                        .map(SerdeRegisterValue::F64)
                        .map_err(|err| err.to_string()),
                    _ => unimplemented!(),
                },
                ValueType::String => record_field
                    .parse::<String>()
                    .map(SerdeRegisterValue::String)
                    .map_err(|err| err.to_string()),
                _ => unimplemented!(),
            };
            let register_value = match parsed_record_field {
                Ok(val) => val,
                Err(err) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        StorageRecordDeserializeError::ParseRegisterValue(anyhow::anyhow!(
                            "{}: register index = {}, register type = {}, recorded value = {}",
                            err,
                            register_index,
                            register_type,
                            record_field
                        )),
                    )
                    .into());
                }
            };
            register_values.push(Some(register_value));
        }
        Ok(StorageRecord {
            created_at_offset_ns,
            observed_at,
            register_values,
        })
    }
}

impl<RegisterValue> From<Record<RegisterValue>> for StorageRecord
where
    RegisterValue: Into<SerdeRegisterValue>,
{
    fn from(from: Record<RegisterValue>) -> Self {
        let Record {
            prelude: RecordPrelude { created_at_offset },
            observation:
                ObservedRegisterValues {
                    observed_at,
                    register_values,
                },
        } = from;
        Self {
            created_at_offset_ns: created_at_offset.into(),
            observed_at: observed_at.into(),
            register_values: register_values
                .into_iter()
                .map(|v| v.map(Into::into))
                .collect(),
        }
    }
}

impl<RegisterValue> From<StorageRecord> for Record<RegisterValue>
where
    RegisterValue: From<SerdeRegisterValue>,
{
    fn from(from: StorageRecord) -> Self {
        let StorageRecord {
            created_at_offset_ns,
            observed_at,
            register_values,
        } = from;
        Self {
            prelude: RecordPrelude {
                created_at_offset: created_at_offset_ns.into(),
            },
            observation: ObservedRegisterValues {
                observed_at: observed_at.into(),
                register_values: register_values
                    .into_iter()
                    .map(|v| v.map(Into::into))
                    .collect(),
            },
        }
    }
}

#[allow(missing_debug_implementations)]
pub struct CsvFileRecordStorage {
    register_types: Vec<ValueType>,
    inner: csv::FileRecordStorageWithDeserializer<StorageRecordDeserializer, StorageRecord>,
}

impl CsvFileRecordStorage {
    pub fn try_new<I>(config: StorageConfig, base_path: PathBuf, registers_iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = (register::Index, ValueType)>,
    {
        let file_name_template = RollingFileNameTemplate {
            prefix: "record_".to_string(),
            suffix: ".csv".to_string(),
        };
        let mut register_types = Vec::new();
        let mut registers = Vec::new();
        let custom_headers = iter::once("created_at".to_string())
            .chain(iter::once("observed_at".to_string()))
            .chain(
                registers_iter
                    .into_iter()
                    .map(|(register_index, register_type)| {
                        register_types.push(register_type);
                        registers.push((register_index, register_type));
                        register_index.to_string()
                    }),
            );
        let inner = csv::FileRecordStorageWithDeserializer::try_new(
            config,
            base_path,
            file_name_template,
            Some(custom_headers.collect()),
            StorageRecordDeserializer { registers },
        )?;
        Ok(Self {
            register_types,
            inner,
        })
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

impl<RegisterValue> RecordStorage<RegisterValue> for CsvFileRecordStorage
where
    RegisterValue: Into<SerdeRegisterValue> + From<SerdeRegisterValue> + ToValueType,
{
    fn append_record(
        &mut self,
        created_at: &SystemTimeInstant,
        record: Record<RegisterValue>,
    ) -> Result<StoredRecordPrelude> {
        for (register_type, register_value) in self
            .register_types
            .iter()
            .zip(record.observation.register_values.iter())
        {
            if let Some(register_value) = register_value {
                if *register_type != register_value.to_value_type() {
                    return Err(Error::MismatchingRegisterTypes {
                        expected: *register_type,
                        actual: register_value.to_value_type(),
                    });
                }
            }
        }
        let (record_written, _) = self
            .inner
            .append_record(created_at, StorageRecord::from(record))?;
        // Silently ignore expected write errors here
        // TODO: How to report them without overwhelming clients??
        if let Err(err) = record_written {
            log::debug!("Failed to append record: {}", err);
        }
        Ok(StoredRecordPrelude::create(created_at.system_time()))
    }

    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<StoredRecord<RegisterValue>>> {
        // TODO: How to avoid conversion of (intermediate) vectors?
        Ok(self.inner.recent_records(limit).map(|v| {
            v.into_iter()
                .map(|(created_at_origin, storage_record)| {
                    StoredRecord::restore(created_at_origin, storage_record.into())
                })
                .collect()
        })?)
    }

    fn filter_records(
        &mut self,
        limit: NonZeroUsize,
        filter: &RecordPreludeFilter,
    ) -> Result<Vec<StoredRecord<RegisterValue>>> {
        Ok(self
            .inner
            .filter_records_by_prelude(limit, filter)
            .map(|v| {
                v.into_iter()
                    .map(|(created_at_origin, storage_record)| {
                        StoredRecord::restore(created_at_origin, storage_record.into())
                    })
                    .collect()
            })?)
    }
}
