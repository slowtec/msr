// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]

#![warn(rust_2018_idioms)]
#![warn(rust_2021_compatibility)]
#![warn(missing_debug_implementations)]
#![warn(unreachable_pub)]
#![warn(unsafe_code)]
#![warn(clippy::all)]
#![warn(clippy::explicit_deref_methods)]
#![warn(clippy::explicit_into_iter_loop)]
#![warn(clippy::explicit_iter_loop)]
#![warn(clippy::must_use_candidate)]
#![warn(rustdoc::broken_intra_doc_links)]
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
