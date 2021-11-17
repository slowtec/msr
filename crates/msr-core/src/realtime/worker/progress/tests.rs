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
    assert_eq!(ProgressHint::Continue, progress_hint.load());

    // Suspend
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspend, progress_hint.load());

    // Suspend again
    assert_eq!(
        SwitchAtomicStateOk::Ignored,
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspend, progress_hint.load());

    // Resume
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Suspend,
        },
        progress_hint.resume().unwrap()
    );
    assert_eq!(ProgressHint::Continue, progress_hint.load());

    // Resume again
    assert_eq!(
        SwitchAtomicStateOk::Ignored,
        progress_hint.resume().unwrap()
    );
    assert_eq!(ProgressHint::Continue, progress_hint.load());

    // Finish while running
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        progress_hint.finish().unwrap()
    );
    assert_eq!(ProgressHint::Finish, progress_hint.load());

    // Finish again
    assert_eq!(
        SwitchAtomicStateOk::Ignored,
        progress_hint.finish().unwrap()
    );
    assert_eq!(ProgressHint::Finish, progress_hint.load());

    // Reset after finished
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Finish,
        },
        progress_hint.reset()
    );
    assert_eq!(ProgressHint::Continue, progress_hint.load());

    // Reset again
    assert_eq!(SwitchAtomicStateOk::Ignored, progress_hint.reset());
    assert_eq!(ProgressHint::Continue, progress_hint.load());

    // Finish while suspended
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        progress_hint.suspend().unwrap()
    );
    assert_eq!(ProgressHint::Suspend, progress_hint.load());
    assert_eq!(
        SwitchAtomicStateOk::Accepted {
            previous_state: ProgressHint::Suspend,
        },
        progress_hint.finish().unwrap()
    );
    assert_eq!(ProgressHint::Finish, progress_hint.load());

    // Reject suspend after finished
    assert_eq!(
        Err(SwitchAtomicStateErr::Rejected {
            current_state: ProgressHint::Finish,
        }),
        progress_hint.suspend()
    );
    assert_eq!(ProgressHint::Finish, progress_hint.load());

    // Reject resume after finished
    assert_eq!(
        Err(SwitchAtomicStateErr::Rejected {
            current_state: ProgressHint::Finish,
        }),
        progress_hint.resume()
    );
    assert_eq!(ProgressHint::Finish, progress_hint.load());
}

#[test]
fn progress_hint_sender_receiver_attach_detach() -> anyhow::Result<()> {
    let mut rx = ProgressHintReceiver::default();

    // Attach and test 1st sender
    let tx1 = ProgressHintSender::attach(&rx);
    assert!(tx1.is_attached());
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        tx1.suspend()?
    );
    assert_eq!(ProgressHint::Suspend, rx.load());

    // Attach and test 2nd sender by cloning the 1st sender
    let tx2 = tx1.clone();
    assert!(tx2.is_attached());
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Suspend,
        },
        tx2.resume()?
    );
    assert_eq!(ProgressHint::Continue, rx.load());

    // Detach the receiver
    rx.detach();

    // Both senders are detached now
    assert!(!tx1.is_attached());
    assert!(!tx2.is_attached());

    // All subsequent attempts to switch the progress hint fail
    assert!(matches!(
        tx1.finish(),
        Err(SwitchProgressHintError::Detached)
    ));
    assert!(matches!(
        tx2.finish(),
        Err(SwitchProgressHintError::Detached)
    ));

    Ok(())
}

#[test]
fn progress_hint_handover_temporal_decoupling_of_sender_receiver() -> anyhow::Result<()> {
    let rx = ProgressHintReceiver::default();

    // No update has been sent yet
    assert!(!rx.wait_until(Instant::now()));

    let tx = ProgressHintSender::attach(&rx);
    assert!(tx.is_attached());
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        tx.finish()?
    );
    assert_eq!(ProgressHint::Finish, rx.load());

    // Drop the sender before the receiver notices the update
    drop(tx);

    // The receiver should notice the pending update by now
    assert!(rx.wait_until(Instant::now()));

    Ok(())
}

#[test]
fn progress_hint_handover_consume_single_update_notification_once() -> anyhow::Result<()> {
    let rx = ProgressHintReceiver::default();
    let tx = ProgressHintSender::attach(&rx);
    assert!(tx.is_attached());

    // No update has been sent yet
    assert!(!rx.wait_until(Instant::now()));

    // Continue -> Suspend
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        tx.suspend()?
    );
    assert_eq!(ProgressHint::Suspend, rx.load());

    // Consume update notification once
    assert!(rx.wait_until(Instant::now()));
    assert!(!rx.wait_until(Instant::now()));

    // Suspend -> Continue
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Suspend,
        },
        tx.resume()?
    );
    assert_eq!(ProgressHint::Continue, rx.load());
    // Continue -> Finish
    assert_eq!(
        SwitchProgressHintOk::Accepted {
            previous_state: ProgressHint::Continue,
        },
        tx.finish()?
    );
    assert_eq!(ProgressHint::Finish, rx.load());

    // Consume single notification once after 2 updates
    assert!(rx.wait_until(Instant::now()));
    assert!(!rx.wait_until(Instant::now()));

    Ok(())
}

#[test]
fn progress_hint_handover_try_switch_without_update_notification() -> anyhow::Result<()> {
    let mut rx = ProgressHintReceiver::default();

    // No update has been sent yet
    assert!(!rx.wait_until(Instant::now()));

    // Continue -> Suspend
    assert!(rx.try_suspending());
    assert_eq!(ProgressHint::Suspend, rx.load());

    // No update notification after try_suspending()
    assert!(!rx.wait_until(Instant::now()));

    // Suspend -> Finish
    assert!(rx.try_finishing());
    assert_eq!(ProgressHint::Finish, rx.load());

    // No update notification after try_finishing()
    assert!(!rx.wait_until(Instant::now()));

    Ok(())
}
