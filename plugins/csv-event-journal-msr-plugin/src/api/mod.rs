use super::*;

// Re-export internal types that are used in the public API
pub use crate::internal::context::{
    Config, EntryNotRecorded, EntryRecorded, RecordEntryOutcome, State, Status,
};

pub mod controller;
pub use self::controller::Controller;

pub mod command;
pub use self::command::Command;

pub mod query;
pub use self::query::Query;

pub mod event;
pub use self::event::Event;

#[derive(Debug)]
pub enum Message {
    Command(Command),
    Query(Query),
}

impl From<Command> for Message {
    fn from(command: Command) -> Self {
        Self::Command(command)
    }
}

impl From<Query> for Message {
    fn from(query: Query) -> Self {
        Self::Query(query)
    }
}
