use std::{num::NonZeroUsize, path::PathBuf, time::SystemTime};

use ::csv::Reader as CsvReader;

use crate::{
    fs::{
        policy::{RollingFileInfo, RollingFileNameTemplate},
        WriteResult,
    },
    storage::{
        self, csv, BinaryDataFormat, CreatedAtOffset, RecordStorageBase, RecordStorageRead,
        RecordStorageWrite, StorageConfig, StorageDescriptor, StorageStatistics,
        MAX_PREALLOCATED_CAPACITY_LIMIT,
    },
    time::SystemInstant,
};

use super::{Error, Record, RecordFilter, RecordStorage, Result, StorageRecord, StoredRecord};

#[allow(missing_debug_implementations)]
pub struct FileRecordStorage {
    inner: csv::FileRecordStorage<StorageRecord, StorageRecord>,
}

impl FileRecordStorage {
    pub fn try_new(
        base_path: PathBuf,
        file_name_prefix: String,
        initial_config: StorageConfig,
    ) -> Result<Self> {
        let file_name_template = RollingFileNameTemplate {
            prefix: file_name_prefix,
            suffix: ".csv".to_string(),
        };
        let inner =
            csv::FileRecordStorage::try_new(initial_config, base_path, file_name_template, None)?;
        Ok(Self { inner })
    }
}

fn filter_map_storage_record(
    created_at_origin: SystemTime,
    record: StorageRecord,
    binary_data_format: BinaryDataFormat,
) -> Option<StoredRecord> {
    match StoredRecord::try_restore(created_at_origin, record, binary_data_format) {
        Ok(record) => Some(record),
        Err(err) => {
            // This should never happen
            log::error!("Failed to convert record: {}", err);
            // Skip and continue
            None
        }
    }
}

impl RecordStorageBase for FileRecordStorage {
    fn config(&self) -> &StorageConfig {
        self.inner.config()
    }

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig {
        self.inner.replace_config(new_config)
    }

    fn descriptor(&self) -> &StorageDescriptor {
        self.inner.descriptor()
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

impl RecordStorageWrite<Record> for FileRecordStorage {
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
        record: Record,
    ) -> storage::Result<(WriteResult, CreatedAtOffset)> {
        let storage_record = StorageRecord::try_new(record, self.config().binary_data_format)?;
        self.inner.append_record(created_at, storage_record)
    }
}

impl RecordStorage for FileRecordStorage {
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
                        filter_map_storage_record(
                            create_at_origin,
                            record,
                            self.config().binary_data_format,
                        )
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
            records = reader_into_filtered_record_iter(
                reader,
                file_info.created_at.into(),
                filter.clone(),
                self.config().binary_data_format,
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

fn reader_into_filtered_record_iter<R>(
    reader: CsvReader<R>,
    created_at_origin: SystemTime,
    filter: RecordFilter,
    binary_data_format: BinaryDataFormat,
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
        .filter_map(move |record| {
            filter_map_storage_record(created_at_origin, record, binary_data_format)
        })
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
