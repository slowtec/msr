use msr_core::event_journal::Entry;

use crate::ResultSender;

use super::{Config, RecordEntryOutcome, State};

#[derive(Debug)]
pub enum Command {
    ReplaceConfig(ResultSender<Config>, Config),
    SwitchState(ResultSender<()>, State),
    RecordEntry(ResultSender<RecordEntryOutcome>, Entry),
    Shutdown(ResultSender<()>),
}
