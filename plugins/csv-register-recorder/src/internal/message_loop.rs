use std::path::{Path, PathBuf};

use tokio::task;

use msr_plugin::{message_channel, send_reply, MessageLoop};

use crate::{
    api::{
        event::{LifecycleEvent, NotificationEvent},
        Command, Config, Event, Message, Query, RegisterGroupId, State,
    },
    EventPubSub, MessageSender, Result,
};

use super::{
    context::{self, Context},
    invoke_context_from_message_loop,
};

struct ContextEventCallback {
    event_pubsub: EventPubSub,
}

impl context::ContextEventCallback for ContextEventCallback {
    fn data_directory_created(&self, register_group_id: &RegisterGroupId, fs_path: &Path) {
        let event = Event::Notification(NotificationEvent::DataDirectoryCreated {
            register_group_id: register_group_id.to_owned(),
            fs_path: fs_path.to_owned(),
        });
        self.event_pubsub.publish_event(event)
    }
}

pub fn create_message_loop(
    data_dir: PathBuf,
    file_name_prefix: String,
    event_pubsub: EventPubSub,
    initial_config: Config,
    initial_state: State,
) -> Result<(MessageLoop, MessageSender)> {
    let (message_tx, mut message_rx) = message_channel();
    let context_events = ContextEventCallback {
        event_pubsub: event_pubsub.clone(),
    };
    let mut context = Context::try_new(
        data_dir,
        file_name_prefix,
        initial_config,
        initial_state,
        Box::new(context_events) as _,
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
                        Command::ReplaceRegisterGroupConfig(
                            reply_tx,
                            register_group_id,
                            new_config,
                        ) => {
                            invoke_context_from_message_loop::command_replace_register_group_config(
                                &mut context,
                                &event_pubsub,
                                reply_tx,
                                register_group_id,
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
                        Command::RecordObservedRegisterGroupValues(
                            reply_tx,
                            register_group_id,
                            observed_register_values,
                        ) => {
                            invoke_context_from_message_loop::command_record_observed_register_group_values(
                                &mut context,
                                reply_tx,
                                register_group_id,
                                observed_register_values,
                            );
                        }
                        Command::Shutdown(reply_tx) => {
                            invoke_context_from_message_loop::command_shutdown(reply_tx);
                            exit_message_loop = true;
                        }
                        Command::SmokeTest(reply_tx) => {
                            // TODO: Remove
                            let response = task::block_in_place(|| context.smoke_test());
                            send_reply(reply_tx, response);
                        }
                    }
                }
                Message::Query(query) => {
                    log::debug!("Received query {:?}", query);
                    match query {
                        Query::Config(reply_tx) => {
                            invoke_context_from_message_loop::query_config(&context, reply_tx);
                        }
                        Query::RegisterGroupConfig(reply_tx, register_group_id) => {
                            invoke_context_from_message_loop::query_register_group_config(
                                &context,
                                reply_tx,
                                &register_group_id,
                            );
                        }
                        Query::Status(reply_tx, request) => {
                            invoke_context_from_message_loop::query_status(
                                &mut context,
                                reply_tx,
                                request,
                            );
                        }
                        Query::RecentRecords(reply_tx, register_group_id, request) => {
                            invoke_context_from_message_loop::query_recent_records(
                                &mut context,
                                reply_tx,
                                &register_group_id,
                                request,
                            );
                        }
                        Query::FilterRecords(reply_tx, register_group_id, request) => {
                            invoke_context_from_message_loop::query_filter_records(
                                &mut context,
                                reply_tx,
                                &register_group_id,
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
