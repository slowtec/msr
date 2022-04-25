use tokio::task;

use msr_core::event_journal::{Entry, Error, StoredRecord};

use msr_plugin::send_reply;

use crate::{
    api::{
        event::{IncidentEvent, LifecycleEvent},
        query, Config, Event, RecordEntryOutcome, State, Status,
    },
    EventPubSub, ResultSender,
};

use super::context::Context;

pub(crate) fn command_replace_config(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<Config>,
    new_config: Config,
) {
    let result = task::block_in_place(|| {
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
    send_reply(reply_tx, result.map_err(Into::into));
}

pub(crate) fn command_switch_state(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<()>,
    new_state: State,
) {
    let result = task::block_in_place(|| {
        context.switch_state(new_state).map_err(|err| {
            log::warn!("Failed to switch state: {}", err);
            err
        })
    })
    .map(|_old_state| {
        let event = Event::Lifecycle(LifecycleEvent::StateChanged(new_state));
        event_pubsub.publish_event(event);
    });
    send_reply(reply_tx, result.map_err(Into::into));
}

pub(crate) fn command_record_entry(
    context: &mut Context,
    event_pubsub: &EventPubSub,
    reply_tx: ResultSender<RecordEntryOutcome>,
    new_entry: Entry,
) {
    let result = task::block_in_place(|| {
        context.record_entry(new_entry).map_err(|err| {
            log::warn!("Failed create new entry: {}", err);
            err
        })
    })
    .map_err(|err| {
        if let Error::Storage(msr_core::storage::Error::Io(err)) = &err {
            let os_code = err.raw_os_error();
            let message = err.to_string();
            let event = Event::Incident(IncidentEvent::IoWriteError { os_code, message });
            event_pubsub.publish_event(event);
        }
        err
    });
    send_reply(reply_tx, result.map_err(Into::into));
}

pub(crate) fn command_shutdown(_context: &mut Context, reply_tx: ResultSender<()>) {
    send_reply(reply_tx, Ok(()));
}

pub(crate) fn query_config(context: &Context, reply_tx: ResultSender<Config>) {
    let result = task::block_in_place(|| Ok(context.config().to_owned()));
    send_reply(reply_tx, result);
}

pub(crate) fn query_status(
    context: &mut Context,
    reply_tx: ResultSender<Status>,
    request: query::StatusRequest,
) {
    let result = task::block_in_place(|| {
        let query::StatusRequest {
            with_storage_statistics,
        } = request;
        context.status(with_storage_statistics).map_err(|err| {
            log::warn!("Failed to query status: {}", err);
            err
        })
    });
    send_reply(reply_tx, result.map_err(Into::into));
}

pub(crate) fn query_recent_records(
    context: &mut Context,
    reply_tx: ResultSender<Vec<StoredRecord>>,
    request: query::RecentRecordsRequest,
) {
    let result = task::block_in_place(|| {
        let query::RecentRecordsRequest { limit } = request;
        context.recent_records(limit).map_err(|err| {
            log::warn!("Failed to query recent records: {}", err);
            err
        })
    });
    send_reply(reply_tx, result.map_err(Into::into));
}

pub(crate) fn query_filter_records(
    context: &mut Context,
    reply_tx: ResultSender<Vec<StoredRecord>>,
    request: query::FilterRecordsRequest,
) {
    let result = task::block_in_place(|| {
        let query::FilterRecordsRequest { limit, filter } = request;
        context.filter_records(limit, filter).map_err(|err| {
            log::warn!("Failed to query filtered records: {}", err);
            err
        })
    });
    send_reply(reply_tx, result.map_err(Into::into));
}
