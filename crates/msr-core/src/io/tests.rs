use super::*;

#[test]
fn counting_write() {
    let (mut writer, bytes_written) = CountingWrite::from_writer(Vec::with_capacity(100));
    assert_eq!(0, bytes_written.value());
    assert!(writer.write(&[1]).is_ok());
    assert_eq!(1, bytes_written.value());
    assert!(writer.write(&[2, 3]).is_ok());
    assert_eq!(3, bytes_written.value());
    assert!(writer.write(&[]).is_ok());
    assert_eq!(3, bytes_written.value());
    assert!(writer.write(&[4, 5, 6, 7]).is_ok());
    assert_eq!(7, bytes_written.value());
}
