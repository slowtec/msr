use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct AtomicFlag(AtomicBool);

impl AtomicFlag {
    pub fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }

    pub fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn check_and_reset(&self) -> bool {
        self.0
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
    }
}
