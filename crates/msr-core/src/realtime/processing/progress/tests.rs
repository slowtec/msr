use super::*;

#[test]
fn atomic_progress_hint_sequence() {
    let progress_hint = AtomicProgressHint::default();
    assert_eq!(ProgressHint::default(), progress_hint.load());
    assert!(progress_hint.suspend());
    assert_eq!(ProgressHint::Suspending, progress_hint.load());
    assert!(progress_hint.resume());
    assert_eq!(ProgressHint::default(), progress_hint.load());
    progress_hint.terminate();
    assert_eq!(ProgressHint::Terminating, progress_hint.load());
    progress_hint.reset();
    assert_eq!(ProgressHint::default(), progress_hint.load());
}
