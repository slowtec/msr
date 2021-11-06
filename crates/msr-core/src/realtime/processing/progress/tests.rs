use super::*;

#[test]
fn atomic_progress_hint_default() {
    assert_eq!(
        ProgressHint::default(),
        AtomicProgressHint::default().load()
    );
}

#[test]
/// Test the behavior of the underlying state machine in isolation (single-threaded)
fn atomic_progress_hint_sequence() {
    let progress_hint = AtomicProgressHint::default();
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Suspend
    assert_eq!(AtomicProgressHintSwitch::Accepted, progress_hint.suspend());
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Suspend again
    assert_eq!(AtomicProgressHintSwitch::Ignored, progress_hint.suspend());
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Resume
    assert_eq!(AtomicProgressHintSwitch::Accepted, progress_hint.resume());
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Resume again
    assert_eq!(AtomicProgressHintSwitch::Ignored, progress_hint.resume());
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while running
    assert_eq!(
        AtomicProgressHintSwitch::Accepted,
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Terminate again
    assert_eq!(AtomicProgressHintSwitch::Ignored, progress_hint.terminate());
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reset after terminated
    assert_eq!(AtomicProgressHintSwitch::Accepted, progress_hint.reset());
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Reset again
    assert_eq!(AtomicProgressHintSwitch::Ignored, progress_hint.reset());
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while suspended
    assert_eq!(AtomicProgressHintSwitch::Accepted, progress_hint.suspend());
    assert_eq!(ProgressHint::Suspending, progress_hint.load());
    assert_eq!(
        AtomicProgressHintSwitch::Accepted,
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject suspend after terminated
    assert_eq!(AtomicProgressHintSwitch::Rejected, progress_hint.suspend());
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject resume after terminated
    assert_eq!(AtomicProgressHintSwitch::Rejected, progress_hint.resume());
    assert_eq!(ProgressHint::Terminating, progress_hint.load());
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(
        WaitForProgressHintSignalOutcome::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)?
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(ProgressHintSwitchOutcome::Accepted, handshake.suspend()?);

    assert_eq!(
        WaitForProgressHintSignalOutcome::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)?
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_max_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(ProgressHintSwitchOutcome::Accepted, handshake.suspend()?);

    assert_eq!(
        WaitForProgressHintSignalOutcome::Signaled,
        handshake.wait_for_signal_with_timeout(Duration::MAX)?
    );

    Ok(())
}
