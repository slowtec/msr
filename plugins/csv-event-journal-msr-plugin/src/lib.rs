// FIXME: Enable all warnings before the release
//#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(test, deny(warnings))]
#![warn(rust_2018_idioms)]

use std::{
    io::Error as IoError,
    num::{NonZeroU32, NonZeroU64},
    path::PathBuf,
};

use thiserror::Error;

use msr_core::{
    event_journal::Severity,
    storage::{MemorySize, StorageConfig, StorageSegmentConfig, TimeInterval},
};

use msr_plugin::EventPublisherIndex;

pub mod api;

mod internal;
use self::internal::message_loop::create_message_loop;

#[derive(Debug, Clone)]
pub struct Environment {
    pub event_publisher_index: EventPublisherIndex,

    /// Directory for storing CSV data
    pub data_dir: PathBuf,

    pub custom_file_name_prefix: Option<String>,
}

pub fn default_storage_config() -> StorageConfig {
    StorageConfig {
        retention_time: TimeInterval::Days(NonZeroU32::new(180).unwrap()), // 180 days
        segmentation: StorageSegmentConfig {
            time_interval: TimeInterval::Days(NonZeroU32::new(1).unwrap()), // daily
            size_limit: MemorySize::Bytes(NonZeroU64::new(1_048_576).unwrap()), // 1 MiB
        },
    }
}

pub fn default_config() -> api::Config {
    api::Config {
        severity_threshold: Severity::Information,
        storage: default_storage_config(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginSetup {
    pub initial_config: api::Config,
    pub initial_state: api::State,
}

impl Default for PluginSetup {
    fn default() -> Self {
        Self {
            initial_config: default_config(),
            initial_state: api::State::Inactive,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("missing config")]
    MissingConfig,

    #[error("invalid state")]
    InvalidState,

    // TODO: Rename this variant?
    #[error(transparent)]
    MsrCore(#[from] msr_core::event_journal::Error),

    #[error(transparent)]
    Io(#[from] IoError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub type PluginError = msr_plugin::PluginError<Error>;
pub type PluginResult<T> = msr_plugin::PluginResult<T, Error>;

pub type MessageSender = msr_plugin::MessageSender<api::Message>;
pub type MessageReceiver = msr_plugin::MessageReceiver<api::Message>;

pub type ResultSender<T> = msr_plugin::ResultSender<T, Error>;
pub type ResultReceiver<T> = msr_plugin::ResultReceiver<T, Error>;

pub type PublishedEvent = msr_plugin::PublishedEvent<api::Event>;
pub type EventReceiver = msr_plugin::EventReceiver<api::Event>;
type EventPubSub = msr_plugin::EventPubSub<api::Event>;

pub type Plugin = msr_plugin::PluginContainer<api::Message, api::Event>;
pub type PluginPorts = msr_plugin::PluginPorts<api::Message, api::Event>;

pub const DEFAULT_FILE_NAME_PREFIX: &str = "event_journal_records_";

pub fn create_plugin(
    environment: Environment,
    plugin_setup: PluginSetup,
    event_channel_capacity: usize,
) -> Result<Plugin> {
    let Environment {
        event_publisher_index,
        data_dir,
        custom_file_name_prefix,
    } = environment;
    let PluginSetup {
        initial_config,
        initial_state,
    } = plugin_setup;
    let (event_pubsub, event_subscriber) =
        EventPubSub::new(event_publisher_index, event_channel_capacity);
    let file_name_prefix =
        custom_file_name_prefix.unwrap_or_else(|| DEFAULT_FILE_NAME_PREFIX.to_owned());
    let (message_loop, message_tx) = create_message_loop(
        data_dir,
        file_name_prefix,
        event_pubsub,
        initial_config,
        initial_state,
    )?;
    Ok(Plugin {
        ports: PluginPorts {
            message_tx,
            event_subscriber,
        },
        message_loop,
    })
}
