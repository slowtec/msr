#![warn(unsafe_code)]
#![cfg_attr(not(debug_assertions), deny(warnings))]
#![deny(rust_2018_idioms)]
#![deny(rust_2021_compatibility)]
// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(clippy::explicit_deref_methods)]
#![deny(clippy::explicit_into_iter_loop)]
#![deny(clippy::explicit_iter_loop)]
#![deny(clippy::must_use_candidate)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

//! Industrial Automation Toolbox - Core Components

mod measure;
mod value;

pub use self::{measure::*, value::*};

pub mod audit;
pub mod control;
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
