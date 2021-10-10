use msr_core::csv_event_journal::Entry;

use super::{Config, RecordEntryOutcome, ResultSender, State};

#[derive(Debug)]
pub enum Command {
    ReplaceConfig(ResultSender<Config>, Config),
    SwitchState(ResultSender<()>, State),
    RecordEntry(ResultSender<RecordEntryOutcome>, Entry),
    Shutdown(ResultSender<()>),
}
