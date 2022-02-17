use std::{
    collections::VecDeque,
    fs::{self, File},
    io::Result as IoResult,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::SystemTime,
};

use csv::{Reader as CsvReader, StringRecord as CsvStringRecord};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    fs::{
        csv::RollingFileWriter,
        policy::{
            FileInfoFilter, RollingFileConfig, RollingFileInfoWithSize, RollingFileLimits,
            RollingFileNameTemplate, RollingFileStatus, RollingFileSystem, SystemTimeRange,
        },
        WriteResult,
    },
    storage::{
        CreatedAtOffset, MemorySize, ReadableRecordPrelude, RecordPreludeFilter, RecordStorageBase,
        RecordStorageRead, RecordStorageWrite, Result, StorageConfig, StorageDescriptor,
        StorageSegmentConfig, StorageSegmentStatistics, StorageStatistics, WritableRecordPrelude,
        MAX_PREALLOCATED_CAPACITY_LIMIT,
    },
    time::{Interval, SystemInstant, Timestamp},
};

fn open_readable_file(file_path: &Path) -> IoResult<File> {
    let mut open_options = fs::OpenOptions::new();
    open_options.read(true).create(false);
    open_options.open(file_path)
}

pub fn file_info_filter_from_record_prelude_filter(filter: RecordPreludeFilter) -> FileInfoFilter {
    let RecordPreludeFilter {
        since_created_at,
        until_created_at,
    } = filter;
    if since_created_at.is_none() && until_created_at.is_none() {
        return Default::default();
    }
    let since_created_at = since_created_at.unwrap_or(SystemTime::UNIX_EPOCH);
    let until_created_at = until_created_at.unwrap_or_else(SystemTime::now);
    FileInfoFilter {
        created_at: Some(SystemTimeRange::InclusiveUpperBound(
            since_created_at..=until_created_at,
        )),
    }
}

pub fn create_file_reader(file_path: &Path) -> IoResult<CsvReader<File>> {
    let file = open_readable_file(file_path)?;
    let mut builder = csv::ReaderBuilder::new();
    Ok(builder
        .has_headers(true)
        .terminator(csv::Terminator::CRLF)
        .from_reader(file))
}

#[derive(Debug)]
pub struct WritingStatus {
    pub rolling_file: RollingFileStatus,
    pub writer: RollingFileWriter,
    pub first_record_created_at: SystemInstant,
    pub last_record_created_at: SystemTime,
    pub flush_pending: bool,
}

impl WritingStatus {
    pub fn flush_before_reading(&mut self) -> Result<()> {
        if self.flush_pending {
            self.writer.flush()?;
        }
        Ok(())
    }
}

#[allow(missing_debug_implementations)]
pub struct FileRecordStorage<RI, RO> {
    config: StorageConfig,

    descriptor: StorageDescriptor,

    custom_header: Option<CsvStringRecord>,

    rolling_file_config: RollingFileConfig,

    writing_status: Option<WritingStatus>,

    _record_in_phantom: std::marker::PhantomData<RI>,

    _record_out_phantom: std::marker::PhantomData<RO>,
}

impl<RI, RO> FileRecordStorage<RI, RO> {
    pub fn flush_before_reading(&mut self) -> Result<()> {
        if let Some(writing_status) = self.writing_status.as_mut() {
            writing_status.flush_before_reading()?;
        }
        Ok(())
    }

    pub fn read_all_dir_entries_filtered_chronologically(
        &self,
        filter: &FileInfoFilter,
    ) -> IoResult<Vec<RollingFileInfoWithSize>> {
        self.rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(filter)
    }

    pub fn try_new(
        config: StorageConfig,
        base_path: PathBuf,
        file_name_template: RollingFileNameTemplate,
        custom_header: Option<CsvStringRecord>,
    ) -> Result<Self> {
        let descriptor = StorageDescriptor {
            kind: "csv-file".to_string(),
            base_path: Some(base_path.clone()),
        };
        let StorageConfig {
            segmentation:
                StorageSegmentConfig {
                    time_interval: segment_time_interval,
                    size_limit: segment_size_limit,
                },
            ..
        } = config;
        Ok(Self {
            config,
            descriptor,
            custom_header,
            rolling_file_config: RollingFileConfig {
                system: RollingFileSystem {
                    base_path,
                    file_name_template,
                },
                limits: RollingFileLimits {
                    max_bytes_written: Some(match segment_size_limit {
                        MemorySize::Bytes(bytes) => bytes.get(),
                    }),
                    interval: Some(segment_time_interval.into()),
                    ..Default::default()
                },
            },
            writing_status: None,
            _record_in_phantom: Default::default(),
            _record_out_phantom: Default::default(),
        })
    }
}

impl<RI, RO> FileRecordStorage<RI, RO>
where
    RI: Serialize,
{
    fn writer(
        &mut self,
        created_at: &SystemInstant,
    ) -> Result<(&mut RollingFileWriter, CreatedAtOffset)> {
        if self.writing_status.is_none() {
            // Perform housekeeping before initially
            self.perform_housekeeping()?;
            let writer = RollingFileWriter::new(
                self.rolling_file_config.clone(),
                self.custom_header.clone(),
            );
            // FIXME: Read created_at time stamp from last record in the file
            // instead of using the time stamp of the file itself (= time stamp
            // of the first record in the file).
            let first_record_created_at = created_at.clone();
            let last_record_created_at = first_record_created_at.system_time();
            self.writing_status = Some(WritingStatus {
                rolling_file: RollingFileStatus::new(last_record_created_at),
                writer,
                first_record_created_at,
                last_record_created_at,
                flush_pending: false,
            });
        }
        // A write operation is supposed to follow immediately
        let writing_status = self.writing_status.as_mut().unwrap();
        if writing_status.last_record_created_at
            < writing_status.first_record_created_at.system_time()
        {
            log::warn!(
                "System time discontinuity between subsequent records detected: {} < {}",
                Timestamp::from(writing_status.last_record_created_at),
                writing_status.first_record_created_at.timestamp_utc(),
            );
        }
        debug_assert!(created_at.instant() >= writing_status.first_record_created_at.instant());
        let created_at_offset =
            created_at.instant() - writing_status.first_record_created_at.instant();
        writing_status.last_record_created_at = created_at.system_time();
        writing_status.flush_pending = true;
        Ok((&mut writing_status.writer, created_at_offset.into()))
    }
}

pub fn reader_into_filtered_record_iter<R, D>(
    reader: CsvReader<R>,
    created_at_origin: SystemTime,
    filter: RecordPreludeFilter,
) -> impl Iterator<Item = D>
where
    R: std::io::Read,
    D: ReadableRecordPrelude + DeserializeOwned,
{
    let RecordPreludeFilter {
        since_created_at,
        until_created_at,
    } = filter;
    reader
        .into_deserialize::<D>()
        .filter_map(|record_result| match record_result {
            Ok(record) => Some(record),
            Err(err) => {
                if err.is_io_error() {
                    log::warn!("Failed to read CSV record: {}", err);
                } else {
                    // This should never happen
                    log::error!("Failed to deserialize CSV record: {}", err);
                }
                // Skip and continue
                None
            }
        })
        .skip_while(move |record| {
            if let Some(since_created_at) = since_created_at {
                record
                    .created_at_offset()
                    .system_time_from_origin(created_at_origin)
                    < since_created_at
            } else {
                false
            }
        })
        .take_while(move |record| {
            if let Some(until_created_at) = until_created_at {
                record
                    .created_at_offset()
                    .system_time_from_origin(created_at_origin)
                    <= until_created_at
            } else {
                true
            }
        })
}

impl<RI, RO> RecordStorageBase for FileRecordStorage<RI, RO> {
    fn descriptor(&self) -> &StorageDescriptor {
        &self.descriptor
    }

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig {
        std::mem::replace(&mut self.config, new_config)
    }

    fn perform_housekeeping(&mut self) -> Result<()> {
        let created_since =
            Interval::from(self.config.retention_time).system_time_before(SystemTime::now());
        self.retain_all_records_created_since(created_since)
    }

    fn retain_all_records_created_since(&mut self, created_since: SystemTime) -> Result<()> {
        self.flush_before_reading()?;
        let mut files_with_entries_created_until = self
            .rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(&FileInfoFilter {
                // Inclusive upper bound, because we will not delete the last entry (see below)
                created_at: Some(SystemTimeRange::InclusiveUpperBound(
                    SystemTime::UNIX_EPOCH..=created_since,
                )),
            })?;
        if files_with_entries_created_until.is_empty() {
            return Ok(());
        }
        // The last file might contain entries that need to be preserved!
        files_with_entries_created_until.pop();
        for file_info in files_with_entries_created_until {
            log::info!("Deleting file {}", file_info.path.display());
            fs::remove_file(&file_info.path)?;
        }
        Ok(())
    }

    fn report_statistics(&mut self) -> Result<StorageStatistics> {
        self.flush_before_reading()?;
        let mut total_records = 0usize;
        let mut total_bytes = 0u64;
        // FIXME: Pre-allocate capacity for segments
        let mut segments = Vec::with_capacity(1024);
        for file_info in &self
            .rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(&Default::default())?
        {
            let reader = create_file_reader(&file_info.path)?;
            let segment_total_records =
                reader.into_byte_records().filter(|res| res.is_ok()).count();
            total_records += segment_total_records;
            let segment_total_bytes = file_info.size_in_bytes;
            let segment = StorageSegmentStatistics {
                created_at: file_info.created_at.into(),
                total_records: segment_total_records,
                total_bytes: Some(segment_total_bytes),
            };
            total_bytes += segment_total_bytes;
            segments.push(segment);
        }
        Ok(StorageStatistics {
            total_records: Some(total_records),
            total_bytes: Some(total_bytes),
            segments: Some(segments),
        })
    }
}

impl<RI, RO> RecordStorageWrite<RI> for FileRecordStorage<RI, RO>
where
    RI: WritableRecordPrelude + Serialize,
{
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
        mut record: RI,
    ) -> Result<(WriteResult, CreatedAtOffset)> {
        let (writer, created_at_offset) = self.writer(created_at)?;
        record.set_created_at_offset(created_at_offset);
        let (record_written, _) = writer.serialize(created_at, created_at_offset.nanos, &record)?;
        Ok((record_written, created_at_offset))
    }
}

impl<RI, RO> RecordStorageRead<RO> for FileRecordStorage<RI, RO>
where
    RO: ReadableRecordPrelude + DeserializeOwned,
{
    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<(SystemTime, RO)>> {
        self.flush_before_reading()?;
        let limit = limit.get().min(MAX_PREALLOCATED_CAPACITY_LIMIT);
        let mut reverse_records = Vec::new();
        let mut recent_files = self
            .rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(&Default::default())?;
        recent_files.reverse();
        for file_info in &recent_files {
            if limit <= reverse_records.len() {
                break;
            }
            let remaining_limit = limit - reverse_records.len();
            let reader = create_file_reader(&file_info.path)?;
            let earlier_records = VecDeque::with_capacity(remaining_limit);
            let earlier_records = reader_into_filtered_record_iter(
                reader,
                file_info.created_at.into(),
                Default::default(),
            )
            .fold(earlier_records, |mut records, record: RO| {
                debug_assert!(records.len() <= limit);
                if records.len() == limit {
                    records.pop_front();
                }
                debug_assert!(records.len() < limit);
                records.push_back((file_info.created_at.into(), record));
                records
            });
            reverse_records.reserve(remaining_limit);
            reverse_records = earlier_records.into_iter().rev().fold(
                reverse_records,
                |mut reverse_records, record| {
                    reverse_records.push(record);
                    reverse_records
                },
            );
        }
        Ok(reverse_records)
    }
}

pub trait StringRecordDeserializer<T> {
    fn deserialize_string_record(&self, record: &CsvStringRecord) -> Result<T>;
}

#[derive(Debug, Clone)]
pub struct StringRecordDeserializeOwned {
    pub headers: Option<CsvStringRecord>,
}

impl<T> StringRecordDeserializer<T> for StringRecordDeserializeOwned
where
    T: DeserializeOwned,
{
    fn deserialize_string_record(&self, record: &CsvStringRecord) -> Result<T> {
        Ok(record.deserialize(self.headers.as_ref())?)
    }
}

#[derive(Debug, Clone)]
pub enum FilteredRecord<T> {
    Match(T),
    MismatchCreatedAfter,
}

pub fn read_next_from_string_record_filtered<R, D, T>(
    reader: &mut CsvReader<R>,
    record: &mut CsvStringRecord,
    deserializer: &D,
    created_at_origin: SystemTime,
    filter: &RecordPreludeFilter,
) -> Result<Option<FilteredRecord<T>>>
where
    R: std::io::Read,
    D: StringRecordDeserializer<T>,
    T: ReadableRecordPrelude,
{
    let RecordPreludeFilter {
        since_created_at,
        until_created_at,
    } = &filter;
    while reader.read_record(record)? {
        let record = deserializer.deserialize_string_record(record)?;
        if let Some(since_created_at) = since_created_at {
            if record
                .created_at_offset()
                .system_time_from_origin(created_at_origin)
                < *since_created_at
            {
                // skip
                continue;
            }
            if let Some(until_created_at) = until_created_at {
                if record
                    .created_at_offset()
                    .system_time_from_origin(created_at_origin)
                    > *until_created_at
                {
                    // done
                    return Ok(Some(FilteredRecord::MismatchCreatedAfter));
                }
            }
        }
        return Ok(Some(FilteredRecord::Match(record)));
    }
    Ok(None)
}

#[allow(missing_debug_implementations)]
pub struct FileRecordStorageWithDeserializer<D, T> {
    inner: FileRecordStorage<T, T>,
    deserializer: D,
}

impl<D, T> FileRecordStorageWithDeserializer<D, T> {
    pub fn try_new(
        config: StorageConfig,
        base_path: PathBuf,
        file_name_template: RollingFileNameTemplate,
        custom_header: Option<CsvStringRecord>,
        deserializer: D,
    ) -> Result<Self> {
        let inner =
            FileRecordStorage::try_new(config, base_path, file_name_template, custom_header)?;
        Ok(Self {
            inner,
            deserializer,
        })
    }

    pub fn flush_before_reading(&mut self) -> Result<()> {
        self.inner.flush_before_reading()
    }

    pub fn read_all_dir_entries_filtered_chronologically(
        &self,
        filter: &FileInfoFilter,
    ) -> IoResult<Vec<RollingFileInfoWithSize>> {
        self.inner
            .read_all_dir_entries_filtered_chronologically(filter)
    }
}

impl<D, T> RecordStorageBase for FileRecordStorageWithDeserializer<D, T> {
    fn descriptor(&self) -> &StorageDescriptor {
        self.inner.descriptor()
    }

    fn replace_config(&mut self, new_config: StorageConfig) -> StorageConfig {
        self.inner.replace_config(new_config)
    }

    fn perform_housekeeping(&mut self) -> Result<()> {
        self.inner.perform_housekeeping()
    }

    fn retain_all_records_created_since(&mut self, created_since: SystemTime) -> Result<()> {
        self.inner.retain_all_records_created_since(created_since)
    }

    fn report_statistics(&mut self) -> Result<StorageStatistics> {
        self.inner.report_statistics()
    }
}

impl<D, T> RecordStorageWrite<T> for FileRecordStorageWithDeserializer<D, T>
where
    T: WritableRecordPrelude + Serialize,
{
    fn append_record(
        &mut self,
        created_at: &SystemInstant,
        record: T,
    ) -> Result<(WriteResult, CreatedAtOffset)> {
        self.inner.append_record(created_at, record)
    }
}

impl<D, T> RecordStorageRead<T> for FileRecordStorageWithDeserializer<D, T>
where
    T: ReadableRecordPrelude,
    D: StringRecordDeserializer<T>,
{
    #[allow(clippy::panic_in_result_fn)] // unreachable!()
    fn recent_records(&mut self, limit: NonZeroUsize) -> Result<Vec<(SystemTime, T)>> {
        self.inner.flush_before_reading()?;
        let limit = limit.get().min(MAX_PREALLOCATED_CAPACITY_LIMIT);
        let mut reverse_records = Vec::new();
        let mut recent_files = self
            .inner
            .rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(&Default::default())?;
        recent_files.reverse();
        let mut record = CsvStringRecord::new();
        let filter = Default::default();
        for file_info in &recent_files {
            if limit <= reverse_records.len() {
                break;
            }
            let remaining_limit = limit - reverse_records.len();
            let mut reader = create_file_reader(&file_info.path)?;
            let mut earlier_records = VecDeque::with_capacity(remaining_limit);
            while let Some(filtered_record) = read_next_from_string_record_filtered(
                &mut reader,
                &mut record,
                &self.deserializer,
                file_info.created_at.into(),
                &filter,
            )? {
                match filtered_record {
                    FilteredRecord::Match(record) => {
                        debug_assert!(earlier_records.len() <= limit);
                        if earlier_records.len() == limit {
                            earlier_records.pop_front();
                        }
                        debug_assert!(earlier_records.len() < limit);
                        earlier_records.push_back((file_info.created_at.into(), record));
                    }
                    FilteredRecord::MismatchCreatedAfter => {
                        unreachable!("not filtered");
                    }
                }
            }
            reverse_records.reserve(remaining_limit);
            reverse_records = earlier_records.into_iter().rev().fold(
                reverse_records,
                |mut reverse_records, record| {
                    reverse_records.push(record);
                    reverse_records
                },
            );
        }
        Ok(reverse_records)
    }
}

impl<D, T> FileRecordStorageWithDeserializer<D, T>
where
    D: StringRecordDeserializer<T>,
    T: ReadableRecordPrelude,
{
    pub fn filter_records_by_prelude(
        &mut self,
        limit: NonZeroUsize,
        filter: &RecordPreludeFilter,
    ) -> Result<Vec<(SystemTime, T)>> {
        self.inner.flush_before_reading()?;
        let limit = limit.get().min(MAX_PREALLOCATED_CAPACITY_LIMIT);
        let mut records = Vec::with_capacity(limit);
        let mut record = CsvStringRecord::new();
        for file_info in self
            .inner
            .rolling_file_config
            .system
            .read_all_dir_entries_filtered_chronologically(
                &file_info_filter_from_record_prelude_filter(filter.clone()),
            )?
        {
            if limit <= records.len() {
                break;
            }
            let mut reader = create_file_reader(&file_info.path)?;
            while let Some(filtered_record) = read_next_from_string_record_filtered(
                &mut reader,
                &mut record,
                &self.deserializer,
                file_info.created_at.into(),
                filter,
            )? {
                if limit <= records.len() {
                    break;
                }
                match filtered_record {
                    FilteredRecord::Match(record) => {
                        records.push((file_info.created_at.into(), record));
                    }
                    FilteredRecord::MismatchCreatedAfter => {
                        return Ok(records);
                    }
                }
            }
        }
        Ok(records.into_iter().collect())
    }
}
