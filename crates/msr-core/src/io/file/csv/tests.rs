use std::time::Duration;

use tempfile::TempDir;

use crate::io::file::policy::{RollingFileNameTemplate, RollingFileSystem};

use super::*;

#[test]
fn write_records_with_max_bytes_written_limit() {
    let temp_dir = TempDir::new().unwrap();
    let config = RollingFileConfig {
        system: RollingFileSystem {
            base_path: temp_dir.path().to_path_buf(),
            file_name_template: RollingFileNameTemplate {
                prefix: "prefix_".into(),
                suffix: "_suffix.csv".into(),
            },
        },
        limits: RollingFileLimits {
            max_bytes_written: Some(5),
            max_records_written: None,
            max_nanoseconds_offset: None,
            interval: None,
        },
    };
    let mut writer = RollingFileWriter::new(config, None);
    assert!(writer.current_file_info().is_none());
    assert_eq!(
        (Ok(()), None),
        writer
            .write_record(&SystemTimeInstant::now(), 0, &["hello", "1.0"])
            .unwrap()
    );
    // Flushing is required to clear the internal buffers and
    // increment the bytes_written counter!
    assert!(writer.flush().is_ok());
    assert!(writer.current_file_info().is_some());
    let initial_file_info = writer.current_file_info().cloned();
    assert!(initial_file_info.is_some());
    let delta_t = Duration::from_secs(1);
    let (record_written, closed_file_info) = writer
        .write_record(
            &(SystemTimeInstant::now() + delta_t),
            delta_t.as_nanos() as u64,
            &["world", "-1.0"],
        )
        .unwrap();
    assert!(record_written.is_ok());
    assert_eq!(initial_file_info.map(ClosedFileInfo), closed_file_info);
    assert!(writer.current_file_info().is_some());
    assert_ne!(
        writer.current_file_info(),
        closed_file_info.map(ClosedFileInfo::into_inner).as_ref()
    );
}

#[test]
fn write_records_with_max_records_written_limits() {
    let temp_dir = TempDir::new().unwrap();
    let config = RollingFileConfig {
        system: RollingFileSystem {
            base_path: temp_dir.path().to_path_buf(),
            file_name_template: RollingFileNameTemplate {
                prefix: "prefix_".into(),
                suffix: "_suffix.csv".into(),
            },
        },
        limits: RollingFileLimits {
            max_bytes_written: None,
            max_records_written: Some(1),
            max_nanoseconds_offset: None,
            interval: None,
        },
    };
    let mut writer = RollingFileWriter::new(config, None);
    assert!(writer.current_file_info().is_none());
    assert_eq!(
        (Ok(()), None),
        writer
            .write_record(&SystemTimeInstant::now(), 0, &["hello", "1.0"])
            .unwrap()
    );
    // Flushing is required to clear the internal buffers and
    // increment the bytes_written counter!
    assert!(writer.current_file_info().is_some());
    let initial_file_info = writer.current_file_info().cloned();
    assert!(initial_file_info.is_some());
    let delta_t = Duration::from_secs(1);
    let (record_written, closed_file_info) = writer
        .write_record(
            &(SystemTimeInstant::now() + delta_t),
            delta_t.as_nanos() as u64,
            &["world", "-1.0"],
        )
        .unwrap();
    assert!(record_written.is_ok());
    assert_eq!(initial_file_info.map(ClosedFileInfo), closed_file_info);
    assert!(writer.current_file_info().is_some());
    assert_ne!(
        writer.current_file_info(),
        closed_file_info.map(ClosedFileInfo::into_inner).as_ref()
    );
}
