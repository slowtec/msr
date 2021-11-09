use std::time::{Duration, Instant};

use crate::sync::Arc;

use super::*;

type UnitRelay = Relay<()>;

const TIMEOUT_FOR_NEXT_NOTIFICATION: Duration = Duration::from_millis(100);

struct CapturedNotifications<T> {
    number_of_notifications: usize,
    last_value: Option<T>,
}

fn capture_notifications_fn<T>(
    relay: Arc<Relay<T>>,
    max_number_of_notifications: usize,
) -> CapturedNotifications<T> {
    let mut number_of_notifications = 0;
    let mut last_value = None;
    while number_of_notifications < max_number_of_notifications {
        let next_value = relay.wait_for(TIMEOUT_FOR_NEXT_NOTIFICATION);
        if next_value.is_none() {
            // Timed out
            break;
        }
        number_of_notifications += 1;
        last_value = next_value;
    }
    CapturedNotifications {
        number_of_notifications,
        last_value,
    }
}

#[test]
fn wait_for_timeout_zero_empty() {
    let relay = UnitRelay::default();

    assert!(relay.wait_for(Duration::ZERO).is_none());
}

#[test]
fn wait_for_timeout_zero_ready() -> anyhow::Result<()> {
    let relay = Relay::default();

    relay.replace_notify_one(());

    assert!(relay.wait_for(Duration::ZERO).is_some());

    Ok(())
}

#[test]
fn wait_for_timeout_max_ready() {
    let relay = Relay::default();

    relay.replace_notify_one(());

    assert!(relay.wait_for(Duration::MAX).is_some());
}

#[test]
fn wait_until_deadline_now_empty() {
    let relay = UnitRelay::default();

    assert!(relay.wait_until(Instant::now()).is_none());
}

#[test]
fn wait_until_deadline_now_ready() {
    let relay = Relay::default();

    relay.replace_notify_one(());

    assert!(relay.wait_until(Instant::now()).is_some());
}

#[test]
fn keep_last_value() {
    let relay = Relay::default();

    let rounds = 10;

    for i in 1..=rounds {
        relay.replace_notify_one(i);
    }

    assert_eq!(Some(rounds), relay.take());
}

#[test]
fn capture_notifications_concurrently() {
    let relay = Arc::new(Relay::default());

    let rounds = 10;

    let thread = std::thread::spawn({
        let relay = Arc::clone(&relay);
        move || capture_notifications_fn(relay, rounds)
    });

    for i in 1..=rounds {
        relay.replace_notify_one(i);
    }

    let CapturedNotifications {
        number_of_notifications,
        last_value,
    } = thread.join().unwrap();

    assert!(number_of_notifications >= 1);
    assert!(number_of_notifications <= rounds);
    assert_eq!(Some(rounds), last_value);
}
