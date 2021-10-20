// FIXME: Enable all warnings before the release
//#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(test, deny(warnings))]
#![warn(rust_2018_idioms)]

mod measure;
mod value;

pub use self::{measure::*, value::*};

pub mod audit;
pub mod control;
pub mod io;
pub mod register;
pub mod storage;
pub mod sync;
pub mod time;

#[cfg(feature = "realtime-worker-thread")]
pub mod realtime;

#[cfg(feature = "event-journal")]
pub mod event_journal;
