use crate::ResultSender;

use super::{ObservedRegisterValues, RegisterGroupId};

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

use super::{Config, RegisterGroupConfig, State};
