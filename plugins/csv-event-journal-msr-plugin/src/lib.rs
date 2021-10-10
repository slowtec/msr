#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

use std::{
    io::Error as IoError,
    num::{NonZeroU32, NonZeroU64},
    path::PathBuf,
};

use thiserror::Error;

use msr_core::{
    csv_event_journal::{Code, Severity},
    storage::{MemorySize, StorageConfig, StorageSegmentConfig, TimeInterval},
};

pub mod api;
use self::api::{Config, State};

mod internal;
use self::internal::message_loop::create_message_loop;

pub const DEFAULT_JOURNAL_SCOPE: &str = "plugin.msr.csv-event-journal";

pub const DEFAULT_EVENT_PUBLISHER_ID: &str = DEFAULT_JOURNAL_SCOPE;

pub const DEFAULT_SEVERITY_THRESHOLD: Severity = Severity::Information;

#[derive(Debug)]
pub struct JournalCodes;

impl JournalCodes {
    const STOPPING: Code = Code(1);
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginSetup {
    pub initial_config: Config,
    pub initial_state: State,
    pub journal_scope: String,
    pub event_publisher_id: EventPublisherId,
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
        severity_threshold: DEFAULT_SEVERITY_THRESHOLD,
        storage: default_storage_config(),
    }
}

impl Default for PluginSetup {
    fn default() -> Self {
        Self {
            initial_config: Config {
                severity_threshold: DEFAULT_SEVERITY_THRESHOLD,
                storage: default_storage_config(),
            },
            initial_state: State::Inactive,
            journal_scope: DEFAULT_JOURNAL_SCOPE.to_owned(),
            event_publisher_id: DEFAULT_EVENT_PUBLISHER_ID.to_owned(),
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
    MsrCore(#[from] msr_core::csv_event_journal::Error),

    #[error(transparent)]
    Io(#[from] IoError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
pub type PluginError = msr_plugin::PluginError<Error>;
pub type PluginResult<T> = msr_plugin::PluginResult<T, Error>;

pub type ResultSender<T> = msr_plugin::ResultSender<T, Error>;
pub type ResultReceiver<T> = msr_plugin::ResultReceiver<T, Error>;

pub type EventPublisherId = String;
pub type PublishedEvent = msr_plugin::PublishedEvent<EventPublisherId, api::Event>;
pub type EventReceiver = msr_plugin::EventReceiver<EventPublisherId, api::Event>;
type EventPubSub = msr_plugin::EventPubSub<EventPublisherId, api::Event>;

pub type Plugin = msr_plugin::PluginContainer<api::Message, EventPublisherId, api::Event>;
pub type PluginPorts = msr_plugin::PluginPorts<api::Message, EventPublisherId, api::Event>;

#[derive(Debug, Clone)]
pub struct Environment {
    pub data_dir: PathBuf,
}

pub fn create_plugin(
    environment: Environment,
    plugin_setup: PluginSetup,
    event_channel_capacity: usize,
) -> Result<Plugin> {
    let PluginSetup {
        initial_config,
        initial_state,
        journal_scope,
        event_publisher_id,
    } = plugin_setup;
    let (event_pubsub, event_subscriber) =
        EventPubSub::new(event_publisher_id, event_channel_capacity);
    let (message_loop, message_tx) = create_message_loop(
        environment,
        initial_config,
        initial_state,
        journal_scope,
        event_pubsub,
    )?;
    Ok(Plugin {
        ports: PluginPorts {
            message_tx,
            event_subscriber,
        },
        message_loop,
    })
}
