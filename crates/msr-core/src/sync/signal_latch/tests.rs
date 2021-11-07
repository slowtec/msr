use super::*;

#[test]
fn wait_with_timeout_zero() -> anyhow::Result<()> {
    let signal_latch = SignalLatch::default();

    assert_eq!(
        WaitForSignalEvent::TimedOut,
        signal_latch.wait_for_signal_with_timeout(Duration::ZERO)
    );

    Ok(())
}

#[test]
fn wait_with_timeout_zero_signaled() -> anyhow::Result<()> {
    let signal_latch = SignalLatch::default();

    signal_latch.raise_notify_one();

    assert_eq!(
        WaitForSignalEvent::TimedOut,
        signal_latch.wait_for_signal_with_timeout(Duration::ZERO)
    );

    Ok(())
}

#[test]
fn wait_with_timeout_max_signaled() -> anyhow::Result<()> {
    let signal_latch = SignalLatch::default();

    signal_latch.raise_notify_one();

    assert_eq!(
        WaitForSignalEvent::Raised,
        signal_latch.wait_for_signal_with_timeout(Duration::MAX)
    );

    Ok(())
}
