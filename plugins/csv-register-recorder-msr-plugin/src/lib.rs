#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

use std::{
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    result::Result as StdResult,
};

use tokio::sync::mpsc;

use msr_core::storage::{
    MemorySize, RecordPreludeFilter, StorageConfig, StorageSegmentConfig, TimeInterval,
};

pub mod api;

mod plugin;
pub use self::plugin::{create_plugin, Plugin, PluginConfig, PluginPorts};

mod context;
pub use self::context::{
    Config, Error, RegisterGroupConfig, RegisterGroupStatus, Result as NewResult, State, Status,
};

pub mod register;
use self::register::{GroupId as RegisterGroupId, ObservedRegisterValues, StoredRecord};

pub type Result<T> = StdResult<T, Error>;
pub type PluginError = msr_plugin::PluginError<Error>;
pub type PluginResult<T> = msr_plugin::PluginResult<T, Error>;

pub type MessageSender = mpsc::UnboundedSender<Message>;

type ResultSender<T> = msr_plugin::ResultSender<T, Error>;
pub type ResultReceiver<T> = msr_plugin::ResultReceiver<T, Error>;

pub type EventPublisherId = String;
pub type PublishedEvent = msr_plugin::PublishedEvent<EventPublisherId, Event>;
pub type EventReceiver = msr_plugin::EventReceiver<EventPublisherId, Event>;
type EventPubSub = msr_plugin::EventPubSub<EventPublisherId, Event>;

pub fn default_storage_config() -> StorageConfig {
    StorageConfig {
        retention_time: TimeInterval::Days(NonZeroU32::new(180).unwrap()), // 180 days
        segmentation: StorageSegmentConfig {
            time_interval: TimeInterval::Days(NonZeroU32::new(1).unwrap()), // daily
            size_limit: MemorySize::Bytes(NonZeroU64::new(1_048_576).unwrap()), // 1 MiB
        },
    }
}

#[derive(Debug)]
pub enum Message {
    Command(Command),
    Query(Query),
}

/// Commands are sent over a separate channel apart from
/// queries. This allows to handle them differently when
/// needed, e.g. process them with higher priority.
#[derive(Debug)]
pub enum Command {
    ReplaceConfig(ResultSender<Config>, Config),
    ReplaceRegisterGroupConfig(
        ResultSender<Option<RegisterGroupConfig>>,
        RegisterGroupId,
        RegisterGroupConfig,
    ),
    SwitchState(ResultSender<()>, State),
    RecordObservedRegisterGroupValues(ResultSender<()>, RegisterGroupId, ObservedRegisterValues),
    Shutdown(ResultSender<()>),
    // TODO: Replace pseudo smoke test command with integration test
    SmokeTest(ResultSender<()>),
}

#[derive(Debug, Clone)]
pub struct RecentRecordsRequest {
    pub limit: NonZeroUsize,
}

#[derive(Debug, Clone)]
pub struct FilterRecordsRequest {
    pub limit: NonZeroUsize,
    pub filter: RecordPreludeFilter,
}

#[derive(Debug, Clone, Default)]
pub struct QueryStatusRequest {
    pub with_register_groups: bool,
    pub with_storage_statistics: bool,
}

#[derive(Debug)]
pub enum Query {
    Config(ResultSender<Config>),
    RegisterGroupConfig(ResultSender<Option<RegisterGroupConfig>>, RegisterGroupId),
    Status(ResultSender<Status>, QueryStatusRequest),
    RecentRecords(
        ResultSender<Vec<StoredRecord>>,
        RegisterGroupId,
        RecentRecordsRequest,
    ),
    FilterRecords(
        ResultSender<Vec<StoredRecord>>,
        RegisterGroupId,
        FilterRecordsRequest,
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Lifecycle(LifecycleEvent),
    Notification(NotificationEvent),
    Incident(IncidentEvent),
}

/// Common lifecycle events
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    Started,
    Stopped,
    ConfigChanged(Config),
    StateChanged(State),
}

/// Regular notifications
#[derive(Debug, Clone)]
pub enum NotificationEvent {
    DataDirectoryCreated {
        register_group_id: RegisterGroupId,
        fs_path: PathBuf,
    },
}

/// Unexpected incidents that might require intervention
#[derive(Debug, Clone)]
pub enum IncidentEvent {
    IoWriteError {
        os_code: Option<i32>,
        message: String,
    },
}

struct ContextEventCallback {
    event_pubsub: EventPubSub,
}

impl context::ContextEventCallback for ContextEventCallback {
    fn data_directory_created(&self, register_group_id: &RegisterGroupId, fs_path: &Path) {
        let event = Event::Notification(NotificationEvent::DataDirectoryCreated {
            register_group_id: register_group_id.to_owned(),
            fs_path: fs_path.to_owned(),
        });
        self.event_pubsub.publish_event(event)
    }
}
