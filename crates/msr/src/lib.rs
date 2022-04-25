#![cfg_attr(not(debug_assertions), deny(warnings))]
// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(clippy::explicit_deref_methods)]
#![deny(clippy::explicit_into_iter_loop)]
#![deny(clippy::explicit_iter_loop)]
#![deny(clippy::must_use_candidate)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

pub use msr_core as core;

#[cfg(feature = "plugin")]
pub use msr_plugin as plugin;
