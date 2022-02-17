use std::{
    num::{NonZeroUsize, ParseIntError},
    result::Result as StdResult,
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    register,
    storage::{
        self, CreatedAtOffset, CreatedAtOffsetNanos, ReadableRecordPrelude, RecordPreludeFilter,
        RecordStorageBase, WritableRecordPrelude,
    },
    time::{SystemInstant, Timestamp},
    ScalarValue, Value, ValueType,
};

#[cfg(feature = "with-csv-register-recorder")]
pub mod csv;

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
    pub observed_at: Timestamp,

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
            observed_at: observed_at.timestamp_utc(),
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
    fn generate_prelude(&self) -> Result<(SystemInstant, RecordPrelude)>;
}

#[derive(Debug)]
pub struct DefaultRecordPreludeGenerator;

impl RecordPreludeGenerator for DefaultRecordPreludeGenerator {
    fn generate_prelude(&self) -> Result<(SystemInstant, RecordPrelude)> {
        Ok((SystemInstant::now(), Default::default()))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StoredRecordPrelude {
    pub created_at: SystemTime,
}

impl StoredRecordPrelude {
    // Only used when a storage backend like CSV is enabled
    #[allow(dead_code)]
    fn create(created_at: SystemTime) -> Self {
        Self { created_at }
    }

    // Only used when a storage backend like CSV is enabled
    #[allow(dead_code)]
    fn restore(created_at_origin: SystemTime, prelude: RecordPrelude) -> Self {
        let created_at = prelude
            .created_at_offset
            .system_time_from_origin(created_at_origin);
        Self { created_at }
    }
}

pub trait RecordStorage<RegisterValue>: RecordStorageBase {
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
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
    // Only used when a storage backend like CSV is enabled
    #[allow(dead_code)]
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

    observed_at: Timestamp,

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

// Only used when a storage backend like CSV is enabled
#[allow(dead_code)]
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
    ParseObservedAt(time::error::Parse),

    #[error(transparent)]
    ParseRegisterValue(anyhow::Error),
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
            observed_at,
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
                observed_at,
                register_values: register_values
                    .into_iter()
                    .map(|v| v.map(Into::into))
                    .collect(),
            },
        }
    }
}
