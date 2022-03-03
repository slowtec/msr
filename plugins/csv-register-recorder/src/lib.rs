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

use std::{
    io::Error as IoError,
    num::{NonZeroU32, NonZeroU64},
    path::PathBuf,
};

use thiserror::Error;

use msr_core::{
    register::recorder::Error as MsrRecordError,
    storage::{
        Error as MsrStorageError, MemorySize, StorageConfig, StorageSegmentConfig, TimeInterval,
    },
};

use msr_plugin::EventPublisherIndex;

pub mod api;
use self::api::Config;

mod internal;
use self::internal::message_loop::create_message_loop;

#[derive(Debug, Clone)]
pub struct Environment {
    pub event_publisher_index: EventPublisherIndex,

    /// Directory for storing CSV data
    pub data_dir: PathBuf,

    pub custom_file_name_prefix: Option<String>,
}

#[must_use]
pub fn default_storage_config() -> StorageConfig {
    StorageConfig {
        retention_time: TimeInterval::Days(NonZeroU32::new(180).unwrap()), // 180 days
        segmentation: StorageSegmentConfig {
            time_interval: TimeInterval::Days(NonZeroU32::new(1).unwrap()), // daily
            size_limit: MemorySize::Bytes(NonZeroU64::new(1_048_576).unwrap()), // 1 MiB
        },
        binary_data_format: Default::default(), // no binary data
    }
}

#[must_use]
pub fn default_config() -> Config {
    Config {
        default_storage: default_storage_config(),
        register_groups: Default::default(),
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
    #[error("register group not configured")]
    RegisterGroupUnknown,

    #[error("invalid data format")]
    DataFormatInvalid,

    #[error(transparent)]
    Io(#[from] IoError),

    #[error(transparent)]
    MsrRecord(#[from] MsrRecordError),

    #[error(transparent)]
    MsrStorage(#[from] MsrStorageError),

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

pub const DEFAULT_FILE_NAME_PREFIX: &str = "register_group_records_";

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
