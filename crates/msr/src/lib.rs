// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]

#![warn(rust_2018_idioms)]
#![warn(rust_2021_compatibility)]
#![warn(missing_debug_implementations)]
#![warn(unreachable_pub)]
#![warn(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(rustdoc::broken_intra_doc_links)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

pub use msr_core as core;

#[cfg(feature = "plugin")]
pub use msr_plugin as plugin;
