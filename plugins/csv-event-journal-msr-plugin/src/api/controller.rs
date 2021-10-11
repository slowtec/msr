use msr_core::csv_event_journal::{Entry, StoredRecord};

use msr_plugin::{reply_channel, send_message_receive_result};

use crate::{MessageSender, PluginResult};

use super::{query, Command, Config, Query, RecordEntryOutcome, State, Status};

/// Remote controller for the plugin
///
/// Wraps the message-based communication with the plugin
/// into asynchronous functions.
#[derive(Debug, Clone)]
pub struct Controller {
    message_tx: MessageSender,
}

impl Controller {
    pub const fn new(message_tx: MessageSender) -> Self {
        Self { message_tx }
    }

    pub async fn command_replace_config(&self, new_config: Config) -> PluginResult<Config> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::ReplaceConfig(reply_tx, new_config);

        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_switch_state(&self, new_state: State) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::SwitchState(reply_tx, new_state);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_record_entry(&self, new_entry: Entry) -> PluginResult<RecordEntryOutcome> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::RecordEntry(reply_tx, new_entry);

        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn command_shutdown(&self) -> PluginResult<()> {
        let (reply_tx, reply_rx) = reply_channel();
        let command = Command::Shutdown(reply_tx);
        send_message_receive_result(command, &self.message_tx, reply_rx).await
    }

    pub async fn query_config(&self) -> PluginResult<Config> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::Config(reply_tx);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_status(&self, request: query::StatusRequest) -> PluginResult<Status> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::Status(reply_tx, request);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_recent_records(
        &self,
        request: query::RecentRecordsRequest,
    ) -> PluginResult<Vec<StoredRecord>> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::RecentRecords(reply_tx, request);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }

    pub async fn query_filter_records(
        &self,
        request: query::FilterRecordsRequest,
    ) -> PluginResult<Vec<StoredRecord>> {
        let (reply_tx, reply_rx) = reply_channel();
        let query = Query::FilterRecords(reply_tx, request);
        send_message_receive_result(query, &self.message_tx, reply_rx).await
    }
}
