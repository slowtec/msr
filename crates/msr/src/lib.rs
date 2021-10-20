// FIXME: Enable all warnings before the release
//#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(test, deny(warnings))]
#![warn(rust_2018_idioms)]

pub use msr_core as core;

#[cfg(feature = "plugin")]
pub use msr_plugin as plugin;
