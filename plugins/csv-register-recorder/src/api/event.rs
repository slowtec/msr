use std::path::PathBuf;

use super::{Config, RegisterGroupId, State};

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
