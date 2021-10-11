use tokio::task;

use msr_plugin::send_reply;

use crate::{
    api::{
        event::LifecycleEvent, query, Config, Event, ObservedRegisterValues, RegisterGroupConfig,
        RegisterGroupId, State, Status, StoredRegisterRecord,
    },
    EventPubSub, ResultSender,
};

use super::context::Context;

pub fn command_replace_config(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<Config>,
    new_config: Config,
) {
    let response = task::block_in_place(|| {
        context.replace_config(new_config.clone()).map_err(|err| {
            log::warn!("Failed to replace configuration: {}", err);
            err
        })
    })
    .map(|old_config| {
        let event = Event::Lifecycle(LifecycleEvent::ConfigChanged(new_config));
        event_pubsub.publish_event(event);
        old_config
    });
    send_reply(reply_tx, response);
}

pub fn command_replace_register_group_config(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<Option<RegisterGroupConfig>>,
    register_group_id: RegisterGroupId,
    new_config: RegisterGroupConfig,
) {
    let response = task::block_in_place(|| {
        context
            .replace_register_group_config(register_group_id.clone(), new_config)
            .map_err(|err| {
                log::warn!(
                    "Failed replace configuration of register group {}: {}",
                    register_group_id,
                    err
                );
                err
            })
    })
    .map(|old_config| {
        let event = Event::Lifecycle(LifecycleEvent::ConfigChanged(context.config().clone()));
        event_pubsub.publish_event(event);
        old_config
    });
    send_reply(reply_tx, response);
}

pub fn command_switch_state(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<()>,
    new_state: State,
) {
    let response = task::block_in_place(|| {
        context.switch_state(new_state).map_err(|err| {
            log::warn!("Failed to switch state: {}", err);
            err
        })
    })
    .map(|_old_state| {
        let event = Event::Lifecycle(LifecycleEvent::StateChanged(new_state));
        event_pubsub.publish_event(event);
    });
    send_reply(reply_tx, response);
}

pub fn command_record_observed_register_group_values(
    context: &mut Context,
    reply_tx: ResultSender<()>,
    register_group_id: RegisterGroupId,
    observed_register_values: ObservedRegisterValues,
) {
    let response = task::block_in_place(|| {
        context
            .record_observed_register_group_values(&register_group_id, observed_register_values)
            .map(|_| ())
            .map_err(|err| {
                log::warn!("Failed record new observation: {}", err);
                err
            })
    });
    send_reply(reply_tx, response);
}

pub fn command_shutdown(reply_tx: ResultSender<()>) {
    send_reply(reply_tx, Ok(()));
}

pub fn query_config(context: &Context, reply_tx: ResultSender<Config>) {
    let response = task::block_in_place(|| Ok(context.config().to_owned()));
    send_reply(reply_tx, response);
}

pub fn query_register_group_config(
    context: &Context,
    reply_tx: ResultSender<Option<RegisterGroupConfig>>,
    register_group_id: &RegisterGroupId,
) {
    let response =
        task::block_in_place(|| Ok(context.register_group_config(register_group_id).cloned()));
    send_reply(reply_tx, response);
}

pub fn query_status(
    context: &mut Context,
    reply_tx: ResultSender<Status>,
    request: query::StatusRequest,
) {
    let response = task::block_in_place(|| {
        let query::StatusRequest {
            with_register_groups,
            with_storage_statistics,
        } = request;
        context
            .status(with_register_groups, with_storage_statistics)
            .map_err(|err| {
                log::warn!("Failed to query status: {}", err);
                err
            })
    });
    send_reply(reply_tx, response);
}

pub fn query_recent_records(
    context: &mut Context,
    reply_tx: ResultSender<Vec<StoredRegisterRecord>>,
    register_group_id: &RegisterGroupId,
    request: query::RecentRecordsRequest,
) {
    let response = task::block_in_place(|| {
        let query::RecentRecordsRequest { limit } = request;
        context
            .recent_records(register_group_id, limit)
            .map_err(|err| {
                log::warn!("Failed to query recent records: {}", err);
                err
            })
    });
    send_reply(reply_tx, response);
}

pub fn query_filter_records(
    context: &mut Context,
    reply_tx: ResultSender<Vec<StoredRegisterRecord>>,
    register_group_id: &RegisterGroupId,
    request: query::FilterRecordsRequest,
) {
    let response = task::block_in_place(|| {
        let query::FilterRecordsRequest { limit, filter } = request;
        context
            .filter_records(register_group_id, limit, &filter)
            .map_err(|err| {
                log::warn!("Failed to query filtered records: {}", err);
                err
            })
    });
    send_reply(reply_tx, response);
}
