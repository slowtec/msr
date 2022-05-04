pub mod atomic;

pub mod relay;
pub use self::relay::Relay;

// loom doesn't provide a drop-in replacement for std::sync::Weak,
// only for std::sync::Arc. Unfortunately, both are needed.
#[allow(unused_imports)]
pub(crate) use std::sync::{Arc, Weak};

#[cfg(loom)]
#[allow(unused_imports)]
pub(crate) use loom::sync::{Condvar, Mutex};

#[cfg(not(loom))]
#[allow(unused_imports)]
pub(crate) use std::sync::{Condvar, Mutex};
