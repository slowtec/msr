use std::{
    cmp::Ordering,
    ffi::{OsStr, OsString},
    fmt, fs,
    io::{Cursor, ErrorKind as IoErrorKind, Result as IoResult},
    ops::{Range, RangeInclusive},
    path::PathBuf,
    str::{from_utf8, FromStr},
    time::SystemTime,
};

use thiserror::Error;
use time::{format_description::FormatItem, macros::format_description, PrimitiveDateTime};

use crate::time::{Interval, Timestamp};

// The full precision of nanoseconds is required to prevent that
// the time stamp in the file name of the next file could be less
// or equal than the time stamp of the last entry in the previous
// file!
// Format: YYYYMMDDThhmmss.nnnnnnnnnZ
const TIME_STAMP_FORMAT: &[FormatItem<'static>] = format_description!(
    "[year repr:full][month repr:numerical][day]T[hour repr:24][minute][second].[subsecond digits:9]Z"
);
const TIME_STAMP_STRING_LEN: usize = 4 + 2 + 2 + 1 + 2 + 2 + 2 + 1 + 9 + 1;

// 1 year, 1 file per day
const PREALLOCATE_NUMBER_OF_DIR_ENTRIES: usize = 365;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RollingFileLimits {
    pub max_bytes_written: Option<u64>,
    pub max_records_written: Option<u64>,
    pub max_nanoseconds_offset: Option<u64>,
    pub interval: Option<Interval>,
}

impl RollingFileLimits {
    #[must_use]
    pub fn daily() -> Self {
        Self {
            interval: Some(Interval::Days(1)),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn weekly() -> Self {
        Self {
            interval: Some(Interval::Weeks(1)),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollingFileStatus {
    pub created_at: SystemTime,
    pub bytes_written: Option<u64>,
    pub records_written: Option<u64>,
}

impl RollingFileStatus {
    #[must_use]
    pub const fn new(created_at: SystemTime) -> Self {
        Self {
            created_at,
            bytes_written: None,
            records_written: None,
        }
    }

    #[must_use]
    pub fn should_roll(
        &self,
        now: SystemTime,
        now_nanoseconds_offset: u64,
        limits: &RollingFileLimits,
    ) -> bool {
        let Self {
            created_at,
            bytes_written,
            records_written,
        } = self;
        let RollingFileLimits {
            max_bytes_written,
            max_records_written,
            max_nanoseconds_offset,
            interval,
        } = limits;
        if let Some(bytes_written) = bytes_written {
            if let Some(max_bytes_written) = max_bytes_written {
                if bytes_written >= max_bytes_written {
                    return true;
                }
            }
        }
        if let Some(records_written) = records_written {
            if let Some(max_records_written) = max_records_written {
                if records_written >= max_records_written {
                    return true;
                }
            }
        }
        if let Some(max_nanoseconds_offset) = max_nanoseconds_offset {
            if now_nanoseconds_offset >= *max_nanoseconds_offset {
                return true;
            }
        }
        if let Some(interval) = interval {
            let next_rollover = interval.system_time_after(*created_at);
            if next_rollover <= now {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RollingFileNameTemplate {
    pub prefix: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct FileNameTimeStamp(SystemTime);

impl From<SystemTime> for FileNameTimeStamp {
    fn from(from: SystemTime) -> Self {
        Self(from)
    }
}

impl From<FileNameTimeStamp> for SystemTime {
    fn from(from: FileNameTimeStamp) -> Self {
        from.0
    }
}

impl FileNameTimeStamp {
    #[must_use]
    pub fn format(&self) -> String {
        let formatted = Timestamp::from(self.0)
            .to_utc()
            .as_ref()
            .format(TIME_STAMP_FORMAT)
            .unwrap_or_default();
        debug_assert_eq!(TIME_STAMP_STRING_LEN, formatted.len());
        formatted
    }

    #[must_use]
    pub fn format_into(&self, output: &mut [u8; TIME_STAMP_STRING_LEN]) -> usize {
        let formatted_len = Timestamp::from(self.0)
            .to_utc()
            .as_ref()
            .format_into(&mut Cursor::new(output.as_mut_slice()), TIME_STAMP_FORMAT)
            .unwrap_or_default();
        debug_assert_eq!(TIME_STAMP_STRING_LEN, formatted_len);
        formatted_len
    }
}

impl fmt::Display for FileNameTimeStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = [0u8; TIME_STAMP_STRING_LEN];
        let formatted_len = self.format_into(&mut output);
        f.write_str(from_utf8(&output.as_slice()[..formatted_len]).unwrap_or_default())
    }
}

impl FromStr for FileNameTimeStamp {
    type Err = time::error::Parse;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let date_time = PrimitiveDateTime::parse(input, TIME_STAMP_FORMAT)?.assume_utc();
        Ok(Self(date_time.into()))
    }
}

impl RollingFileNameTemplate {
    #[must_use]
    pub fn format_os_string_with_time_stamp(&self, ts: FileNameTimeStamp) -> OsString {
        let Self { prefix, suffix } = self;
        // Reserve 2 bytes per character (Windows/UTF-16) for the time stamp infix
        let infix_capacity = TIME_STAMP_STRING_LEN * 2;
        let mut file_name = OsString::with_capacity(prefix.len() + infix_capacity + suffix.len());
        file_name.push(prefix);
        file_name.push(ts.to_string());
        file_name.push(suffix);
        debug_assert!(file_name.len() <= file_name.capacity());
        file_name
    }

    pub fn parse_time_stamp_from_file_name(
        &self,
        file_name: &OsStr,
    ) -> Result<FileNameTimeStamp, FileNameError> {
        let Self { prefix, suffix } = self;
        let file_name = file_name.to_str().ok_or(FileNameError::Pattern)?;
        // TODO: Replace with strip_prefix/strip_suffix when available
        if !file_name.starts_with(prefix) || !file_name.ends_with(suffix) {
            return Err(FileNameError::Pattern);
        }
        let (_, without_prefix) = file_name.split_at(prefix.len());
        let (ts, _) = without_prefix.split_at(without_prefix.len() - suffix.len());
        if ts.len() != TIME_STAMP_STRING_LEN {
            return Err(FileNameError::Pattern);
        }
        Ok(ts.parse()?)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RollingFileSystem {
    pub base_path: PathBuf,
    pub file_name_template: RollingFileNameTemplate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollingFileInfo {
    pub path: PathBuf,
    pub created_at: FileNameTimeStamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollingFileInfoWithSize {
    pub path: PathBuf,
    pub created_at: FileNameTimeStamp,
    pub size_in_bytes: u64,
}

impl RollingFileInfoWithSize {
    #[must_use]
    pub fn new(info: RollingFileInfo, size_in_bytes: u64) -> Self {
        let RollingFileInfo { path, created_at } = info;
        Self {
            path,
            created_at,
            size_in_bytes,
        }
    }

    fn cmp_created_at(&self, other: &Self) -> Ordering {
        self.created_at.cmp(&other.created_at)
    }
}

impl From<RollingFileInfoWithSize> for RollingFileInfo {
    fn from(from: RollingFileInfoWithSize) -> Self {
        let RollingFileInfoWithSize {
            path,
            created_at,
            size_in_bytes: _,
        } = from;
        Self { path, created_at }
    }
}

#[derive(Debug)]
pub enum OpenRollingFile {
    Opened(fs::File, RollingFileInfo),
    AlreadyExists(PathBuf),
}

#[derive(Error, Debug)]
pub enum FileNameError {
    #[error("unrecognized file name pattern")]
    Pattern,

    #[error(transparent)]
    Parse(#[from] time::error::Parse),
}

#[derive(Clone, Debug)]
pub enum SystemTimeRange {
    OnlyMostRecent,
    ExclusiveUpperBound(Range<SystemTime>),
    InclusiveUpperBound(RangeInclusive<SystemTime>),
}

#[derive(Clone, Debug, Default)]
pub struct FileInfoFilter {
    pub created_at: Option<SystemTimeRange>,
}

impl RollingFileSystem {
    #[must_use]
    pub fn new_file_path(&self, created_at: FileNameTimeStamp) -> PathBuf {
        debug_assert!(PathBuf::from(self.file_name_template.prefix.clone()).is_relative());
        let new_name = self
            .file_name_template
            .format_os_string_with_time_stamp(created_at);
        debug_assert_eq!(
            PathBuf::from(new_name.clone()).is_relative(),
            PathBuf::from(self.file_name_template.prefix.clone()).is_relative()
        );
        debug_assert!(self.base_path.is_dir());
        let mut new_file_path = self.base_path.clone();
        new_file_path.push(new_name);
        new_file_path
    }

    pub fn open_new_file_for_writing(
        &self,
        created_at: FileNameTimeStamp,
    ) -> IoResult<OpenRollingFile> {
        let mut open_options = fs::OpenOptions::new();
        open_options.write(true).create_new(true);
        let path = self.new_file_path(created_at);
        match open_options.open(&path) {
            Ok(file) => {
                let info = RollingFileInfo { path, created_at };
                Ok(OpenRollingFile::Opened(file, info))
            }
            Err(e) => {
                if e.kind() == IoErrorKind::AlreadyExists {
                    Ok(OpenRollingFile::AlreadyExists(path))
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Read all entries in the base path directory
    ///
    /// The matching entries are returned in no particular order.
    pub fn read_all_dir_entries_filtered(
        &self,
        filter: &FileInfoFilter,
    ) -> IoResult<Vec<RollingFileInfoWithSize>> {
        let capacity = if let Some(SystemTimeRange::OnlyMostRecent) = filter.created_at {
            1
        } else {
            PREALLOCATE_NUMBER_OF_DIR_ENTRIES
        };
        let mut entries = Vec::with_capacity(capacity);
        let mut first_created_at_filtered = None; // only used for filtering
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(created_at) = path.file_name().and_then(|file_name| {
                    self.file_name_template
                        .parse_time_stamp_from_file_name(file_name)
                        .ok()
                }) {
                    if let Some(filter_created_at) = &filter.created_at {
                        let filter_created_at_start = match filter_created_at {
                            SystemTimeRange::OnlyMostRecent => {
                                if created_at.0 >= first_created_at_filtered.unwrap_or(created_at.0)
                                {
                                    entries.clear();
                                }
                                created_at.0
                            }
                            SystemTimeRange::ExclusiveUpperBound(filter_created_at) => {
                                if created_at.0 >= filter_created_at.end {
                                    continue;
                                }
                                filter_created_at.start
                            }
                            SystemTimeRange::InclusiveUpperBound(filter_created_at) => {
                                if created_at.0 > *filter_created_at.end() {
                                    continue;
                                }
                                *filter_created_at.start()
                            }
                        };
                        if let Some(first_created_at) = first_created_at_filtered {
                            debug_assert!(first_created_at <= filter_created_at_start);
                            if created_at.0 < first_created_at {
                                continue;
                            }
                        }
                        if created_at.0 <= filter_created_at_start {
                            first_created_at_filtered = Some(created_at.0);
                        }
                    }
                    let size_in_bytes = path.metadata()?.len();
                    entries.push(RollingFileInfoWithSize {
                        path,
                        created_at,
                        size_in_bytes,
                    });
                    continue;
                }
            }
            log::debug!("Ignoring directory entry {}", path.display());
        }
        if let Some(first_created_at_filtered) = first_created_at_filtered {
            // Post-process filter
            entries.retain(|file_info| file_info.created_at.0 >= first_created_at_filtered);
        }
        Ok(entries)
    }

    /// Read all entries in the base path directory, sorted by _created at_ in ascending order
    pub fn read_all_dir_entries_filtered_chronologically(
        &self,
        filter: &FileInfoFilter,
    ) -> IoResult<Vec<RollingFileInfoWithSize>> {
        let mut entries = self.read_all_dir_entries_filtered(filter)?;
        entries.sort_unstable_by(RollingFileInfoWithSize::cmp_created_at);
        Ok(entries)
    }

    pub fn read_most_recent_dir_entry(&self) -> IoResult<Option<RollingFileInfoWithSize>> {
        Ok(self
            .read_all_dir_entries_filtered_chronologically(&FileInfoFilter {
                created_at: Some(SystemTimeRange::OnlyMostRecent),
            })?
            .into_iter()
            .last())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RollingFileConfig {
    pub system: RollingFileSystem,
    pub limits: RollingFileLimits,
}

#[cfg(test)]
mod tests;
