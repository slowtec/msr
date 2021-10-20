use super::*;

#[test]
fn new_atomic_progress_hint_should_have_default_value() {
    assert_eq!(ProgressHint::default(), AtomicProgressHint::new().load());
}
