use super::*;

#[test]
fn atomic_progress_hint_default() {
    assert_eq!(
        ProgressHint::default(),
        AtomicProgressHint::default().peek()
    );
}

#[test]
/// Test the behavior of the underlying state machine in isolation (single-threaded)
fn atomic_progress_hint_sequence() {
    let progress_hint = AtomicProgressHint::default();
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Suspend
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Suspend again
    assert_eq!(
        SwitchAtomicStateOk::Ignored,
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());

    // Resume
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Suspending,
        },
        progress_hint.resume().unwrap()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Resume again
    assert_eq!(
        SwitchAtomicStateOk::Ignored,
        progress_hint.resume().unwrap()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while running
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Terminate again
    assert_eq!(SwitchAtomicStateOk::Ignored, progress_hint.terminate());
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reset after terminated
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Terminating,
        },
        progress_hint.reset()
    );
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Reset again
    assert_eq!(SwitchAtomicStateOk::Ignored, progress_hint.reset());
    assert_eq!(ProgressHint::Running, progress_hint.load());

    // Terminate while suspended
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspending, progress_hint.load());
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Suspending,
        },
        progress_hint.terminate()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject suspend after terminated
    assert_eq!(
        Err(SwitchAtomicStateErr::Rejected {
            current_state: ProgressHint::Terminating,
        }),
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());

    // Reject resume after terminated
    assert_eq!(
        Err(SwitchAtomicStateErr::Rejected {
            current_state: ProgressHint::Terminating,
        }),
        progress_hint.resume()
    );
    assert_eq!(ProgressHint::Terminating, progress_hint.load());
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(
        WaitForProgressHintSignalEvent::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_zero_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        handshake.suspend()?
    );

    assert_eq!(
        WaitForProgressHintSignalEvent::TimedOut,
        handshake.wait_for_signal_with_timeout(Duration::ZERO)
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_wait_for_signal_with_timeout_max_signaled() -> anyhow::Result<()> {
    let handshake = ProgressHintHandshake::default();

    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        handshake.suspend()?
    );

    assert_eq!(
        WaitForProgressHintSignalEvent::Signaled,
        handshake.wait_for_signal_with_timeout(Duration::MAX)
    );

    Ok(())
}

#[test]
fn progress_hint_handshake_sender_receiver() -> anyhow::Result<()> {
    let mut rx = ProgressHintReceiver::default();

    // Attach and test 1st sender
    let tx1 = ProgressHintSender::attach(&rx);
    assert!(tx1.is_attached());
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Running,
        },
        tx1.suspend()?
    );
    assert_eq!(ProgressHint::Suspending, rx.load());

    // Attach and test 2nd sender by cloning the 1st sender
    let tx2 = tx1.clone();
    assert!(tx2.is_attached());
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Suspending,
        },
        tx2.resume()?
    );
    assert_eq!(ProgressHint::Running, rx.load());

    // Detach the receiver
    rx.detach();

    // Both senders are detached now
    assert!(!tx1.is_attached());
    assert!(!tx2.is_attached());

    // All subsequent attempts to switch the progress hint fail
    assert!(matches!(
        tx1.terminate(),
        Err(SwitchProgressHintError::Detached)
    ));
    assert!(matches!(
        tx2.terminate(),
        Err(SwitchProgressHintError::Detached)
    ));

    Ok(())
}
