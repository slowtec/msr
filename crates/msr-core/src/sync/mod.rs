pub mod atomic;

pub mod relay;
pub use self::relay::Relay;

// loom doesn't provide a drop-in replacement for std::sync::Weak,
// only for std::sync::Arc. Unfortunately, both are needed.
#[allow(unused_imports)]
pub(crate) use std::sync::{Arc, Weak};

// loom only provides drop-in replacements for the std::sync
// primitives, but unfortunately not for the parking_lot
// variants that are using a different API.
#[allow(unused_imports)]
pub(crate) use parking_lot::{const_mutex, Condvar, Mutex};
