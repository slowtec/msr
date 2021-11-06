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
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Suspend again
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Ignored,
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Resume
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.resume()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Resume again
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Ignored,
        progress_hint.resume()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while running
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Terminate again
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Ignored,
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reset after terminated
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.reset()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Reset again
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Ignored,
        progress_hint.reset()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while suspended
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Accepted,
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject suspend after terminated
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Rejected,
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject resume after terminated
    assert_eq!(
        SwitchAtomicProgressHintOutcome::Rejected,
        progress_hint.resume()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(
        WaitForProgressHintSignalOk::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)?
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(SwitchProgressHintOk::Accepted, handshake.suspend()?);

    assert_eq!(
        WaitForProgressHintSignalOk::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)?
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_max_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(SwitchProgressHintOk::Accepted, handshake.suspend()?);

    assert_eq!(
        WaitForProgressHintSignalOk::Signaled,
        handshake.wait_for_signal_with_timeout(Duration::MAX)?
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_sender_receiver() {
    let mut rx = ProgressHintReceiver::default();

    // 1st Sender: Success
    let tx1 = ProgressHintSender::attach(&rx);
    assert!(tx1.is_attached());
    assert!(matches!(tx1.suspend(), Ok(SwitchProgressHintOk::Accepted)));
    assert_eq!(ProgressHint::Suspending, rx.load());

    // 2nd (cloned) Sender: Success
    let tx2 = tx1.clone();
    assert!(tx2.is_attached());
    assert!(matches!(tx2.resume(), Ok(SwitchProgressHintOk::Accepted)));
    assert_eq!(ProgressHint::Running, rx.load());

    // Detach the receiver
    rx.detach();

    // Both senders detached now
    assert!(!tx1.is_attached());
    assert!(matches!(
        tx1.terminate(),
        Err(SwitchProgressHintError::Detached)
    ));
    assert!(!tx2.is_attached());
    assert!(matches!(
        tx2.terminate(),
        Err(SwitchProgressHintError::Detached)
    ));
}
