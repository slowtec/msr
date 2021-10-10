use msr_plugin::{reply_channel, send_message_receive_result};

use super::*;

pub use super::Message;

pub async fn command_replace_config(
    message_tx: &MessageSender,
    new_config: Config,
) -> PluginResult<Config> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::ReplaceConfig(reply_tx, new_config));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn command_replace_register_group_config(
    message_tx: &MessageSender,
    register_group_id: RegisterGroupId,
    new_config: RegisterGroupConfig,
) -> PluginResult<Option<RegisterGroupConfig>> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::ReplaceRegisterGroupConfig(
        reply_tx,
        register_group_id,
        new_config,
    ));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn command_switch_state(
    message_tx: &MessageSender,
    new_state: State,
) -> PluginResult<()> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::SwitchState(reply_tx, new_state));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn command_record_observed_register_group_values(
    message_tx: &MessageSender,
    register_group_id: RegisterGroupId,
    observed_register_values: ObservedRegisterValues,
) -> PluginResult<()> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::RecordObservedRegisterGroupValues(
        reply_tx,
        register_group_id,
        observed_register_values,
    ));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn command_shutdown(message_tx: &MessageSender) -> PluginResult<()> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::Shutdown(reply_tx));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn command_smoke_test(message_tx: &MessageSender) -> PluginResult<()> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Command(Command::SmokeTest(reply_tx));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn query_config(message_tx: &MessageSender) -> PluginResult<Config> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Query(Query::Config(reply_tx));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn query_register_group_config(
    message_tx: &MessageSender,
    register_group_id: RegisterGroupId,
) -> PluginResult<Option<RegisterGroupConfig>> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Query(Query::RegisterGroupConfig(reply_tx, register_group_id));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn query_status(
    message_tx: &MessageSender,
    request: QueryStatusRequest,
) -> PluginResult<Status> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Query(Query::Status(reply_tx, request));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn query_recent_records(
    message_tx: &MessageSender,
    register_group_id: RegisterGroupId,
    req: RecentRecordsRequest,
) -> PluginResult<Vec<StoredRecord>> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Query(Query::RecentRecords(reply_tx, register_group_id, req));
    send_message_receive_result(message, message_tx, reply_rx).await
}

pub async fn query_filter_records(
    message_tx: &MessageSender,
    register_group_id: RegisterGroupId,
    req: FilterRecordsRequest,
) -> PluginResult<Vec<StoredRecord>> {
    let (reply_tx, reply_rx) = reply_channel();
    let message = Message::Query(Query::FilterRecords(reply_tx, register_group_id, req));
    send_message_receive_result(message, message_tx, reply_rx).await
}
