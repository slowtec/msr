use super::*;

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

/// Regular notifications for informational purposes
#[derive(Debug, Clone)]
pub struct NotificationEvent {
    // Empty placeholder
}

/// Unexpected incidents that might require (manual) intervention
#[derive(Debug, Clone)]
pub enum IncidentEvent {
    IoWriteError {
        os_code: Option<i32>,
        message: String,
    },
}
