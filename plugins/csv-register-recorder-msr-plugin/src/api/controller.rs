use msr_plugin::{reply_channel, send_message_receive_result};

use crate::{MessageSender, PluginResult};

use super::{
    query, Command, Config, ObservedRegisterValues, Query, RegisterGroupConfig, RegisterGroupId,
    State, Status, StoredRegisterRecord,
};

/// Remote controller for the plugin
///
/// Wraps the message-based communication with the plugin
/// into asynchronous functions.
#[derive(Debug, Clone)]
pub struct Controller {
    message_tx: MessageSender,
}

impl Controller {
    #[must_use]
    pub const fn new(message_tx: MessageSender) -> Self {
        Self { message_tx }
    }

    pub async fn command_replace_config(&self, new_config: Config) -> PluginResult<Config> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::ReplaceConfig(reply_tx, new_config);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_replace_register_group_config(
        &self,
        register_group_id: RegisterGroupId,
        new_config: RegisterGroupConfig,
    ) -> PluginResult<Option<RegisterGroupConfig>> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::ReplaceRegisterGroupConfig(reply_tx, register_group_id, new_config);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_switch_state(&self, new_state: State) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::SwitchState(reply_tx, new_state);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_record_observed_register_group_values(
        &self,
        register_group_id: RegisterGroupId,
        observed_register_values: ObservedRegisterValues,
    ) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::RecordObservedRegisterGroupValues(
            reply_tx,
            register_group_id,
            observed_register_values,
        );
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_shutdown(&self) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::Shutdown(reply_tx);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_smoke_test(&self) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::SmokeTest(reply_tx);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn query_config(&self) -> PluginResult<Config> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::Config(reply_tx);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_register_group_config(
        &self,
        register_group_id: RegisterGroupId,
    ) -> PluginResult<Option<RegisterGroupConfig>> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::RegisterGroupConfig(reply_tx, register_group_id);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_status(&self, request: query::StatusRequest) -> PluginResult<Status> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::Status(reply_tx, request);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_recent_records(
        &self,
        register_group_id: RegisterGroupId,
        req: query::RecentRecordsRequest,
    ) -> PluginResult<Vec<StoredRegisterRecord>> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::RecentRecords(reply_tx, register_group_id, req);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_filter_records(
        &self,
        register_group_id: RegisterGroupId,
        req: query::FilterRecordsRequest,
    ) -> PluginResult<Vec<StoredRegisterRecord>> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::FilterRecords(reply_tx, register_group_id, req);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }
}
