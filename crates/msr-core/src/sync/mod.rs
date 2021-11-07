pub mod atomic;

pub mod signal_latch;
pub use self::signal_latch::{SignalLatch, WaitForSignalEvent};

// loom doesn't provide a drop-in replacement for std::sync::Weak,
// only for std::sync::Arc. Unfortunately, both are needed.
pub(crate) use std::sync::{Arc, Weak};

// loom only provides drop-in replacements for the std::sync
// primitives, but unfortunately not for the parking_lot
// variants that are using a different API.
pub(crate) use parking_lot::{const_mutex, Condvar, Mutex};
