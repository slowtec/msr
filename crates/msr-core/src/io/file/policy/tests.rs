use std::{
    path::{Path, MAIN_SEPARATOR},
    time::Duration,
};

use super::*;

fn verify_file_path(
    actual_file_path: &Path,
    rolling_fs: &RollingFileSystem,
    created_at: FileNameTimeStamp,
) {
    let RollingFileSystem {
        base_path,
        file_name_template,
    } = rolling_fs;
    let actual_file_name = actual_file_path.file_name().unwrap();
    assert_eq!(
        created_at,
        file_name_template
            .parse_time_stamp_from_file_name(actual_file_name)
            .unwrap()
    );
    let RollingFileNameTemplate {
        prefix: file_name_prefix,
        suffix: file_name_suffix,
    } = file_name_template;
    let actual_file_path_str = actual_file_path.to_str().unwrap();
    let base_path_str = base_path.to_str().unwrap();
    assert!(actual_file_path_str.starts_with(base_path_str));
    assert!(actual_file_path_str.contains(MAIN_SEPARATOR));
    assert!(actual_file_path_str.contains(file_name_prefix));
    let created_at = Timestamp::from(SystemTime::from(created_at));
    let expected_file_name =format!(
            "{prefix}{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}.{nanosecond:09}Z{suffix}",
            prefix = file_name_prefix,
            year = created_at.year(),
            month = created_at.month() as u8,
            day = created_at.day(),
            hour = created_at.hour(),
            minute = created_at.minute(),
            second = created_at.second(),
            nanosecond = created_at.nanosecond(),
            suffix = file_name_suffix,
        );
    assert!(actual_file_path_str.ends_with(&expected_file_name));
    assert_eq!(
        actual_file_path_str.find(MAIN_SEPARATOR).unwrap(),
        actual_file_path_str.find(base_path_str).unwrap() + base_path_str.len()
    );
    assert_eq!(
        actual_file_path_str.find(file_name_prefix).unwrap(),
        actual_file_path_str.find(MAIN_SEPARATOR).unwrap() + MAIN_SEPARATOR.to_string().len()
    );
    assert_eq!(
        actual_file_path_str.find(file_name_suffix).unwrap(),
        actual_file_path_str.find(file_name_prefix).unwrap()
            + file_name_prefix.len()
            + TIME_STAMP_STRING_LEN
    );
}

#[test]
fn format_file_name_from_config() {
    let cfg = RollingFileSystem {
        base_path: ".".into(),
        file_name_template: RollingFileNameTemplate {
            prefix: "prefix_".into(),
            suffix: "_suffix.ext".into(),
        },
    };

    let created_at =
        SystemTime::from(Timestamp::parse_rfc3339("1978-01-02T23:04:05.12345678Z").unwrap()).into();
    let file_path = cfg.new_file_path(created_at);
    verify_file_path(&file_path, &cfg, created_at);

    let created_at = SystemTime::now().into();
    let file_path = cfg.new_file_path(created_at);
    verify_file_path(&file_path, &cfg, created_at);
}

#[test]
fn file_info_cmp_created_at_later() {
    let now = SystemTime::now();
    let earlier = RollingFileInfoWithSize {
        path: Default::default(),
        created_at: now.into(),
        size_in_bytes: 0,
    };
    let later = RollingFileInfoWithSize {
        path: Default::default(),
        created_at: (now + Duration::from_secs(1)).into(),
        size_in_bytes: 0,
    };
    assert_eq!(Ordering::Less, earlier.cmp_created_at(&later));
    assert_eq!(Ordering::Equal, earlier.cmp_created_at(&earlier));
    assert_eq!(Ordering::Equal, later.cmp_created_at(&later));
    assert_eq!(Ordering::Greater, later.cmp_created_at(&earlier));
}
