use msr_plugin::MessageLoop;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};

mod api;
mod internal;

pub use api::*;

pub struct Plugin {
    message_loop: MessageLoop,
    message_tx: mpsc::UnboundedSender<Message>,
    broadcast_tx: broadcast::Sender<Event>,
}

#[derive(Debug, Error)]
pub enum SetupError {}

impl Plugin {
    pub fn setup() -> Result<Self, SetupError> {
        let (broadcast_tx, _) = broadcast::channel(100);
        let event_tx = broadcast_tx.clone();
        let (message_loop, message_tx) = internal::create_message_loop(event_tx);
        Ok(Self {
            message_tx,
            message_loop,
            broadcast_tx,
        })
    }
}

impl msr_plugin::Plugin for Plugin {
    type Message = Message;
    type Event = Event;
    fn message_sender(&self) -> mpsc::UnboundedSender<Self::Message> {
        self.message_tx.clone()
    }
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event> {
        self.broadcast_tx.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}
