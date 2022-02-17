use std::{io, iter, num::NonZeroUsize, path::PathBuf, time::SystemTime};

use ::csv::StringRecord as CsvStringRecord;

use crate::{
    fs::policy::RollingFileNameTemplate,
    register,
    storage::{
        self, csv, CreatedAtOffsetNanos, RecordPreludeFilter, RecordStorageBase, RecordStorageRead,
        RecordStorageWrite, StorageConfig, StorageDescriptor, StorageStatistics,
    },
    time::SystemInstant,
    ScalarType, ToValueType, ValueType,
};

use super::*;

impl csv::StringRecordDeserializer<StorageRecord> for StorageRecordDeserializer {
    #[allow(clippy::panic_in_result_fn)] // unimplemented!()
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
        let observed_at = Timestamp::parse_rfc3339(record_fields.next().unwrap())
            .map_err(StorageRecordDeserializeError::ParseObservedAt)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
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

#[allow(missing_debug_implementations)]
pub struct FileRecordStorage {
    register_types: Vec<ValueType>,
    inner: csv::FileRecordStorageWithDeserializer<StorageRecordDeserializer, StorageRecord>,
}

const FILE_NAME_SUFFIX: &str = ".csv";

const CREATED_AT_COLUMN_HEADER: &str = "created_at";
const OBSERVED_AT_COLUMN_HEADER: &str = "observed_at";

impl FileRecordStorage {
    pub fn try_new<I>(
        config: StorageConfig,
        base_path: PathBuf,
        file_name_prefix: String,
        registers_iter: I,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = (register::Index, ValueType)>,
    {
        let file_name_template = RollingFileNameTemplate {
            prefix: file_name_prefix,
            suffix: FILE_NAME_SUFFIX.to_owned(),
        };
        let mut register_types = Vec::new();
        let mut registers = Vec::new();
        let custom_headers = iter::once(CREATED_AT_COLUMN_HEADER.to_owned())
            .chain(iter::once(OBSERVED_AT_COLUMN_HEADER.to_owned()))
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

impl RecordStorageBase for FileRecordStorage {
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

impl<RegisterValue> RecordStorage<RegisterValue> for FileRecordStorage
where
    RegisterValue: Into<SerdeRegisterValue> + From<SerdeRegisterValue> + ToValueType,
{
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
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
