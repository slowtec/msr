use std::{
    fs::File,
    io::{Error as IoError, ErrorKind as IoErrorKind},
    result::Result as StdResult,
    time::SystemTime,
};

use ::csv::{
    Error as CsvError, ErrorKind, StringRecord, Writer as CsvWriter,
    WriterBuilder as CsvWriterBuilder,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    io::{BytesWritten, CountingWrite},
    time::SystemInstant,
};

use super::{
    policy::{
        OpenRollingFile, RollingFileConfig, RollingFileInfo, RollingFileInfoWithSize,
        RollingFileLimits, RollingFileStatus as PolicyRollingFileStatus,
    },
    WriteError, WriteResult,
};

type CountingFileWriter = CsvWriter<CountingWrite<File>>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] IoError),

    #[error(transparent)]
    Csv(#[from] CsvError),
}

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
struct RollingFileStatus {
    created_at: SystemTime,
    // Due to internal buffering the number of bytes that have
    // already been written may not change with every write
    // request. It is only a rough estimate!
    bytes_written: BytesWritten,
    records_written: u64,
}

impl RollingFileStatus {
    fn new(created_at: SystemTime, bytes_written: BytesWritten) -> Self {
        debug_assert_eq!(0, bytes_written.value());
        Self {
            created_at,
            bytes_written,
            records_written: 0,
        }
    }

    fn should_roll(
        &self,
        now: SystemTime,
        now_nanoseconds_offset: u64,
        limits: &RollingFileLimits,
    ) -> bool {
        PolicyRollingFileStatus::from(self).should_roll(now, now_nanoseconds_offset, limits)
    }
}

impl From<&RollingFileStatus> for PolicyRollingFileStatus {
    fn from(from: &RollingFileStatus) -> Self {
        let RollingFileStatus {
            created_at,
            bytes_written,
            records_written,
        } = from;
        Self {
            created_at: *created_at,
            bytes_written: Some(bytes_written.value()),
            records_written: Some(*records_written),
        }
    }
}

#[derive(Debug)]
struct RollingFile {
    info: RollingFileInfo,
    status: RollingFileStatus,
    writer: CountingFileWriter,
    last_os_error_code: Option<i32>,
}

impl RollingFile {
    // Custom handling and transformation of I/O errors
    #[allow(clippy::panic_in_result_fn)] // unreachable!()
    fn after_record_written(&mut self, res: StdResult<(), ::csv::Error>) -> Result<WriteResult> {
        match res {
            Ok(()) => {
                self.status.records_written += 1;
                // No error -> Reset last OS error
                self.last_os_error_code = None;
                Ok(Ok(()))
            }
            Err(err) => {
                if let ErrorKind::Io(err) = err.kind() {
                    let last_os_error_code = err.raw_os_error();
                    if let Some(last_os_error_code) = last_os_error_code {
                        if self.last_os_error_code == Some(last_os_error_code) {
                            // Only use log level Debug here to avoid spamming the application log!
                            log::debug!("Writing of CSV record failed repeatedly with I/O error (OS code = {}): {}", last_os_error_code, err);
                            return Ok(Err(WriteError::RepeatedOsError {
                                code: last_os_error_code,
                            }));
                        } else {
                            log::warn!(
                                "Writing of CSV record failed with I/O error (OS code = {}): {}",
                                last_os_error_code,
                                err
                            );
                            // Remember last OS error to suppress repeated errors in the future
                            self.last_os_error_code = Some(last_os_error_code);
                        }
                    } else {
                        log::warn!("Writing of CSV record failed with I/O error: {}", err);
                        // No OS error -> Reset last OS error
                        self.last_os_error_code = None;
                    }
                } else {
                    log::warn!("Writing of CSV record failed: {}", err);
                    return Err(Error::Csv(err));
                }
                match err.into_kind() {
                    ErrorKind::Io(err) => Err(Error::Io(err)),
                    _ => {
                        unreachable!();
                    }
                }
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ClosedFileInfo(RollingFileInfo);

impl ClosedFileInfo {
    #[must_use]
    pub fn into_inner(self) -> RollingFileInfo {
        self.0
    }
}

#[derive(Debug)]
pub struct RollingFileWriter {
    config: RollingFileConfig,
    custom_header: Option<StringRecord>,
    current_file: Option<RollingFile>,
}

impl RollingFileWriter {
    #[must_use]
    pub fn new(config: RollingFileConfig, custom_header: Option<StringRecord>) -> Self {
        Self {
            config,
            custom_header,
            current_file: None,
        }
    }

    fn start_new_file(&self, starting_at: SystemInstant) -> Result<Option<RollingFile>> {
        let new_file = self
            .config
            .system
            .open_new_file_for_writing(starting_at.system_time().into())?;
        match new_file {
            OpenRollingFile::Opened(file, info) => {
                let (writer, bytes_written) = CountingWrite::from_writer(file);
                let status = RollingFileStatus::new(info.created_at.into(), bytes_written);
                let rolling_file = RollingFile {
                    info,
                    status,
                    writer: CsvWriterBuilder::new()
                        .has_headers(self.custom_header.is_none())
                        .from_writer(writer),
                    last_os_error_code: None,
                };
                Ok(Some(rolling_file))
            }
            OpenRollingFile::AlreadyExists(path) => {
                log::info!("File already exists: {}", path.display());
                Ok(None)
            }
        }
    }

    fn roll_file_now(&mut self, now: SystemInstant) -> Result<Option<ClosedFileInfo>> {
        let new_file = self.start_new_file(now)?;
        if let Some(new_file) = new_file {
            log::info!("Opened new file: {}", new_file.info.path.display());
            let old_file = self.current_file.replace(new_file);
            let closed_file_info = old_file.map(|old_file| {
                log::info!("Closing old file: {}", old_file.info.path.display());
                ClosedFileInfo(old_file.info)
            });
            Ok(closed_file_info)
        } else {
            Ok(None)
        }
    }

    /// Query information about the current file.
    #[must_use]
    pub fn current_file_info(&self) -> Option<&RollingFileInfo> {
        self.current_file
            .as_ref()
            .map(|current_file| &current_file.info)
    }

    /// Query information about the current file including (estimated) file size.
    ///
    /// The returned estimated size only reflects the actual size of the file after
    /// cached records has been flushed to disk!
    #[must_use]
    pub fn current_file_info_with_size(&self) -> Option<RollingFileInfoWithSize> {
        self.current_file.as_ref().map(|current_file| {
            let RollingFileInfo { path, created_at } = &current_file.info;
            RollingFileInfoWithSize {
                path: path.clone(),
                created_at: *created_at,
                size_in_bytes: current_file.status.bytes_written.value(),
            }
        })
    }

    pub fn recent_files(&self) -> Result<Vec<RollingFileInfoWithSize>> {
        let mut files = self
            .config
            .system
            .read_all_dir_entries_filtered_chronologically(&Default::default())?;
        files.reverse();
        Ok(files)
    }

    fn before_writing(
        &mut self,
        now: &SystemInstant,
        now_nanoseconds_offset: u64,
    ) -> Result<Option<ClosedFileInfo>> {
        let (closed_file_info, created_new_file) = if let Some(current_file) = &self.current_file {
            if current_file.status.should_roll(
                now.system_time(),
                now_nanoseconds_offset,
                &self.config.limits,
            ) {
                // Try to flush all buffered contents before closing the current file.
                self.flush()?;
                let closed_file_info = self.roll_file_now(now.clone())?;
                let created_new_file = closed_file_info.is_some();
                (closed_file_info, created_new_file)
            } else {
                (None, false)
            }
        } else {
            (self.roll_file_now(now.clone())?, true)
        };
        if let Some(current_file) = self.current_file.as_mut() {
            if created_new_file {
                if let Some(custom_header) = &self.custom_header {
                    current_file.writer.write_record(custom_header)?;
                }
            }
            Ok(closed_file_info)
        } else {
            Err(IoError::new(IoErrorKind::NotFound, "no open file").into())
        }
    }

    /// Write a single record
    pub fn write_record<I, T>(
        &mut self,
        now: &SystemInstant,
        now_nanoseconds_offset: u64,
        record: I,
    ) -> Result<(WriteResult, Option<ClosedFileInfo>)>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        let closed_file_info = self.before_writing(now, now_nanoseconds_offset)?;
        let record_written = if let Some(current_file) = self.current_file.as_mut() {
            let res = current_file.writer.write_record(record);
            current_file.after_record_written(res)?
        } else {
            Err(WriteError::NoFile)
        };
        Ok((record_written, closed_file_info))
    }

    /// Serialize a single record
    pub fn serialize<S: Serialize>(
        &mut self,
        now: &SystemInstant,
        now_nanoseconds_offset: u64,
        record: S,
    ) -> Result<(WriteResult, Option<ClosedFileInfo>)> {
        let closed_file_info = self.before_writing(now, now_nanoseconds_offset)?;
        let record_written = if let Some(current_file) = self.current_file.as_mut() {
            let res = current_file.writer.serialize(record);
            current_file.after_record_written(res)?
        } else {
            Err(WriteError::NoFile)
        };
        Ok((record_written, closed_file_info))
    }

    /// Flush all written records to disk, clearing the internal cache
    pub fn flush(&mut self) -> Result<()> {
        if let Some(current_file) = self.current_file.as_mut() {
            current_file.writer.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
