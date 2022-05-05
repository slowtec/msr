// FIXME: Enable `deny(missing_docs)` before release
//#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

pub use msr_core as core;

#[cfg(feature = "plugin")]
pub use msr_plugin as plugin;
