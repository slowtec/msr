#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

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

pub fn create_plugin(
    environment: Environment,
    plugin_setup: PluginSetup,
    event_channel_capacity: usize,
) -> Result<Plugin> {
    let PluginSetup {
        initial_config,
        initial_state,
    } = plugin_setup;
    let (event_pubsub, event_subscriber) =
        EventPubSub::new(environment.event_publisher_index, event_channel_capacity);
    let (message_loop, message_tx) = create_message_loop(
        environment.data_dir,
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
