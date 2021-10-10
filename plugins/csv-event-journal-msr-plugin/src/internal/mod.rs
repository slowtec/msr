use tokio::sync::mpsc;

use crate::api::Message;

pub mod context;
pub mod invoke_context_from_plugin;
pub mod message_loop;

pub type MessageSender = mpsc::UnboundedSender<Message>;
