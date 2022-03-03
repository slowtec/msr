use std::path::PathBuf;

use msr_core::storage::BinaryDataFormat;
use msr_plugin::{message_channel, MessageLoop};

use crate::{
    api::{event::LifecycleEvent, Command, Config, Event, Message, Query, State},
    EventPubSub, MessageSender, Result,
};

use super::{context::Context, invoke_context_from_message_loop};

pub fn create_message_loop(
    data_dir: PathBuf,
    file_name_prefix: String,
    event_pubsub: EventPubSub,
    binary_data_format: BinaryDataFormat,
    initial_config: Config,
    initial_state: State,
) -> Result<(MessageLoop, MessageSender)> {
    let (message_tx, mut message_rx) = message_channel();
    let mut context = Context::try_new(
        data_dir,
        file_name_prefix,
        binary_data_format,
        initial_config,
        initial_state,
    )?;
    let message_loop = async move {
        let mut exit_message_loop = false;
        log::info!("Starting message loop");
        event_pubsub.publish_event(Event::Lifecycle(LifecycleEvent::Started));
        while let Some(msg) = message_rx.recv().await {
            match msg {
                Message::Command(command) => {
                    log::trace!("Received command {:?}", command);
                    match command {
                        Command::ReplaceConfig(reply_tx, new_config) => {
                            invoke_context_from_message_loop::command_replace_config(
                                &mut context,
                                &event_pubsub,
                                reply_tx,
                                new_config,
                            );
                        }
                        Command::SwitchState(reply_tx, new_state) => {
                            invoke_context_from_message_loop::command_switch_state(
                                &mut context,
                                &event_pubsub,
                                reply_tx,
                                new_state,
                            );
                        }
                        Command::RecordEntry(reply_tx, new_entry) => {
                            invoke_context_from_message_loop::command_record_entry(
                                &mut context,
                                &event_pubsub,
                                reply_tx,
                                new_entry,
                            );
                        }
                        Command::Shutdown(reply_tx) => {
                            invoke_context_from_message_loop::command_shutdown(
                                &mut context,
                                reply_tx,
                            );
                            exit_message_loop = true;
                        }
                    }
                }
                Message::Query(query) => {
                    log::debug!("Received query {:?}", query);
                    match query {
                        Query::Config(reply_tx) => {
                            invoke_context_from_message_loop::query_config(&context, reply_tx);
                        }
                        Query::Status(reply_tx, request) => {
                            invoke_context_from_message_loop::query_status(
                                &mut context,
                                reply_tx,
                                request,
                            );
                        }
                        Query::RecentRecords(reply_tx, request) => {
                            invoke_context_from_message_loop::query_recent_records(
                                &mut context,
                                reply_tx,
                                request,
                            );
                        }
                        Query::FilterRecords(reply_tx, request) => {
                            invoke_context_from_message_loop::query_filter_records(
                                &mut context,
                                reply_tx,
                                request,
                            );
                        }
                    }
                }
            }
            if exit_message_loop {
                log::info!("Exiting message loop");
                break;
            }
        }
        log::info!("Message loop terminated");
        event_pubsub.publish_event(Event::Lifecycle(LifecycleEvent::Stopped));
    };
    Ok((Box::pin(message_loop), message_tx))
}
