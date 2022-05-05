// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

//! Industrial Automation Toolbox - Common core components

mod measure;
mod value;

pub use self::{measure::*, value::*};

pub mod audit;
pub mod control;
pub mod fs;
pub mod io;
pub mod register;
pub mod storage;
pub mod sync;
pub mod thread;
pub mod time;

#[cfg(feature = "realtime-worker-thread")]
pub mod realtime;

#[cfg(feature = "event-journal")]
pub mod event_journal;
