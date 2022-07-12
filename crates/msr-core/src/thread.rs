use std::time::Instant;

pub use std::thread::*;

/// Puts the current thread to sleep for at least until the deadline
///
/// The thread may sleep longer than the deadline specified due to scheduling
/// specifics or platform-dependent functionality. It will never sleep less.
///
/// See also: <https://doc.rust-lang.org/std/thread/fn.sleep.html>
///
/// TODO: Use [spin-sleep](https://github.com/alexheretic/spin-sleep) depending
/// on the use case for reliable accuracy to limit the maximum jitter?
pub fn sleep_until(deadline: Instant) {
    let now = Instant::now();
    if now >= deadline {
        return;
    }
    sleep(deadline.duration_since(now));
}
