use std::time::Duration;

type Relay = super::Relay<()>;

#[test]
fn wait_for_timeout_zero_empty() -> anyhow::Result<()> {
    let relay = Relay::default();

    assert!(relay.wait_for(Duration::ZERO).is_none());

    Ok(())
}

#[test]
fn wait_for_timeout_zero_ready() -> anyhow::Result<()> {
    let relay = Relay::default();

    relay.replace_notify_one(());

    assert!(relay.wait_for(Duration::ZERO).is_some());

    Ok(())
}

#[test]
fn wait_for_timeout_max_ready() -> anyhow::Result<()> {
    let relay = Relay::default();

    relay.replace_notify_one(());

    assert!(relay.wait_for(Duration::MAX).is_some());

    Ok(())
}
