use std::path::{Path, PathBuf};

use tokio::task;

use msr_plugin::{message_channel, send_reply};

use crate::register::GroupId as RegisterGroupId;

use super::{context::Context, *};

mod handlers;

pub const DEFAULT_JOURNAL_SCOPE: &str = "slowrt.plugin.msr.recorder";

pub const DEFAULT_EVENT_PUBLISHER_ID: &str = DEFAULT_JOURNAL_SCOPE;

#[derive(Debug, Clone, PartialEq)]
pub struct PluginConfig {
    pub initial_config: Config,
    pub event_publisher_id: EventPublisherId,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            initial_config: Config {
                default_storage: default_storage_config(),
                register_groups: Default::default(),
            },
            event_publisher_id: DEFAULT_EVENT_PUBLISHER_ID.to_owned(),
        }
    }
}

pub type Plugin = msr_plugin::PluginContainer<Message, EventPublisherId, Event>;
pub type PluginPorts = msr_plugin::PluginPorts<Message, EventPublisherId, Event>;

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

pub fn create_plugin(
    data_path: PathBuf,
    initial_config: PluginConfig,
    initial_state: State,
    event_channel_capacity: usize,
) -> NewResult<Plugin> {
    let PluginConfig {
        event_publisher_id,
        initial_config,
    } = initial_config;
    let (message_tx, mut message_rx) = message_channel();
    let (event_pubsub, event_subscriber) =
        EventPubSub::new(event_publisher_id, event_channel_capacity);
    let context_events = ContextEventCallback {
        event_pubsub: event_pubsub.clone(),
    };
    let mut context = Context::try_new(
        data_path,
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
                            handlers::command_replace_config(
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
                            handlers::command_replace_register_group_config(
                                &mut context,
                                &event_pubsub,
                                reply_tx,
                                register_group_id,
                                new_config,
                            );
                        }
                        Command::SwitchState(reply_tx, new_state) => {
                            handlers::command_switch_state(
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
                            handlers::command_record_observed_register_group_values(
                                &mut context,
                                reply_tx,
                                register_group_id,
                                observed_register_values,
                            );
                        }
                        Command::Shutdown(reply_tx) => {
                            handlers::command_shutdown(reply_tx);
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
                            handlers::query_config(&context, reply_tx);
                        }
                        Query::RegisterGroupConfig(reply_tx, register_group_id) => {
                            handlers::query_register_group_config(
                                &context,
                                reply_tx,
                                &register_group_id,
                            );
                        }
                        Query::Status(reply_tx, request) => {
                            handlers::query_status(&mut context, reply_tx, request);
                        }
                        Query::RecentRecords(reply_tx, register_group_id, request) => {
                            handlers::query_recent_records(
                                &mut context,
                                reply_tx,
                                &register_group_id,
                                request,
                            );
                        }
                        Query::FilterRecords(reply_tx, register_group_id, request) => {
                            handlers::query_filter_records(
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
    Ok(Plugin {
        ports: PluginPorts {
            message_tx,
            event_subscriber,
        },
        message_loop: Box::pin(message_loop),
    })
}
