use std::{error::Error as StdError, fmt, future::Future, pin::Pin};

use thiserror::Error;
use tokio::sync::{
    broadcast,
    mpsc::{self, error::SendError},
    oneshot,
};

use msr_core::audit::Activity;

// ------ -------
//  Plugin shape
// ------ -------

pub trait Plugin {
    type Message;
    type Event;
    fn message_sender(&self) -> MessageSender<Self::Message>;
    fn subscribe_events(&self) -> BroadcastReceiver<Self::Event>;
    fn run(self) -> MessageLoop;
}

#[allow(missing_debug_implementations)]
pub struct PluginContainer<M, E> {
    pub ports: PluginPorts<M, E>,
    pub message_loop: MessageLoop,
}

impl<M, E> Plugin for PluginContainer<M, E> {
    type Message = M;
    type Event = PublishedEvent<E>;

    fn message_sender(&self) -> MessageSender<Self::Message> {
        self.ports.message_tx.clone()
    }
    fn subscribe_events(&self) -> BroadcastReceiver<Self::Event> {
        self.ports.event_subscriber.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}

pub type MessageLoop = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[allow(missing_debug_implementations)]
pub struct PluginPorts<M, E> {
    pub message_tx: MessageSender<M>,
    pub event_subscriber: EventSubscriber<E>,
}

#[derive(Error, Debug)]
pub enum PluginError<E: StdError> {
    #[error("communication error")]
    Communication,

    #[error("internal error: {0}")]
    Internal(E),
}

pub type PluginResult<T, E> = Result<T, PluginError<E>>;

// ------ -------
//   Messages
// ------ -------

// TODO: Use bounded channels for backpressure?
pub type MessageSender<T> = mpsc::UnboundedSender<T>;
pub type MessageReceiver<T> = mpsc::UnboundedReceiver<T>;

pub fn message_channel<T>() -> (MessageSender<T>, MessageReceiver<T>) {
    mpsc::unbounded_channel()
}

// ------ -------
// Reply messages
// ------ -------

pub type ReplySender<T> = oneshot::Sender<T>;
pub type ReplyReceiver<T> = oneshot::Receiver<T>;

pub fn reply_channel<T>() -> (oneshot::Sender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}

pub type ResultSender<T, E> = ReplySender<Result<T, E>>;
pub type ResultReceiver<T, E> = ReplyReceiver<Result<T, E>>;

// ------ -------
//  Broadcasting
// ------ -------

type BroadcastSender<T> = broadcast::Sender<T>;
type BroadcastReceiver<T> = broadcast::Receiver<T>;

#[derive(Debug, Clone)]
pub struct BroadcastSubscriber<T> {
    sender: BroadcastSender<T>,
}

impl<T> BroadcastSubscriber<T> {
    pub fn new(sender: BroadcastSender<T>) -> Self {
        Self { sender }
    }

    pub fn subscribe(&self) -> BroadcastReceiver<T> {
        self.sender.subscribe()
    }
}

pub fn broadcast_channel<T>(channel_capacity: usize) -> (BroadcastSender<T>, BroadcastSubscriber<T>)
where
    T: Clone,
{
    let (tx, _) = broadcast::channel(channel_capacity);
    let subscriber = BroadcastSubscriber::new(tx.clone());
    (tx, subscriber)
}

// ----- ------
//    Events
// ----- ------

/// Internal index into a lookup table with event publisher metadata
pub type EventPublisherIndexValue = usize;

/// Numeric identifier of an event publisher control cycle
///
/// Uniquely identifies an event publisher in the system at runtime.
///
/// The value is supposed to be used as a key or index to retrieve
/// extended metadata for an event publisher that does not need to
/// be sent with every event. This metadata is probably immutable.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventPublisherIndex(EventPublisherIndexValue);

impl EventPublisherIndex {
    pub const fn from_value(value: EventPublisherIndexValue) -> Self {
        Self(value)
    }

    pub const fn to_value(self) -> EventPublisherIndexValue {
        self.0
    }
}

impl From<EventPublisherIndexValue> for EventPublisherIndex {
    fn from(from: EventPublisherIndexValue) -> Self {
        Self::from_value(from)
    }
}

impl From<EventPublisherIndex> for EventPublisherIndexValue {
    fn from(from: EventPublisherIndex) -> Self {
        from.to_value()
    }
}

#[derive(Debug, Clone)]
pub struct PublishedEvent<E> {
    pub published: Activity<EventPublisherIndex>,
    pub payload: E,
}

pub type EventSender<E> = broadcast::Sender<PublishedEvent<E>>;
pub type EventReceiver<E> = broadcast::Receiver<PublishedEvent<E>>;
pub type EventSubscriber<E> = BroadcastSubscriber<PublishedEvent<E>>;

pub fn event_channel<E>(channel_capacity: usize) -> (EventSender<E>, EventSubscriber<E>)
where
    E: Clone,
{
    broadcast_channel(channel_capacity)
}

#[derive(Debug, Clone)]
pub struct EventPubSub<E> {
    publisher_index: EventPublisherIndex,
    event_tx: EventSender<E>,
}

impl<E> EventPubSub<E>
where
    E: fmt::Debug + Clone,
{
    pub fn new(
        publisher_index: impl Into<EventPublisherIndex>,
        channel_capacity: usize,
    ) -> (Self, EventSubscriber<E>) {
        let (event_tx, event_subscriber) = event_channel(channel_capacity);
        (
            Self {
                publisher_index: publisher_index.into(),
                event_tx,
            },
            event_subscriber,
        )
    }

    pub fn publish_event(&self, payload: E) {
        let published = Activity::now(self.publisher_index);
        let event = PublishedEvent { published, payload };
        self.dispatch_event(event);
    }
}

pub trait EventDispatcher<E> {
    fn dispatch_event(&self, event: E);
}

impl<E> EventDispatcher<PublishedEvent<E>> for EventPubSub<E>
where
    E: fmt::Debug + Clone,
{
    fn dispatch_event(&self, event: PublishedEvent<E>) {
        if let Err(event) = self.event_tx.send(event) {
            // Ignore all send errors that are expected if no subscribers
            // are connected.
            log::debug!("No subscribers for published event {:?}", event);
        }
    }
}

// --------- -----------
//   Utility functions
// --------- -----------

pub fn send_message<M, E>(
    message: impl Into<M>,
    message_tx: &MessageSender<M>,
) -> PluginResult<(), E>
where
    M: fmt::Debug,
    E: StdError,
{
    message_tx.send(message.into()).map_err(|send_error| {
        let SendError(message) = send_error;
        log::error!("Unexpected send error: Dropping message {:?}", message);
        PluginError::Communication
    })
}

pub fn send_reply<R>(reply_tx: ReplySender<R>, reply: impl Into<R>)
where
    R: fmt::Debug,
{
    if let Err(reply) = reply_tx.send(reply.into()) {
        // Not an error, may occur if the receiver side has already been dropped
        log::info!("Unexpected send error: Dropping reply {:?}", reply);
    }
}

pub async fn receive_reply<R, E>(reply_rx: ReplyReceiver<R>) -> PluginResult<R, E>
where
    E: StdError,
{
    reply_rx.await.map_err(|receive_error| {
        log::error!("No reply received: {}", receive_error);
        PluginError::Communication
    })
}

pub async fn send_message_receive_reply<M, R, E>(
    message: impl Into<M>,
    message_tx: &MessageSender<M>,
    reply_rx: ReplyReceiver<R>,
) -> PluginResult<R, E>
where
    M: fmt::Debug,
    E: StdError,
{
    send_message(message, message_tx)?;
    receive_reply(reply_rx).await
}

pub async fn receive_result<R, E>(result_rx: ResultReceiver<R, E>) -> PluginResult<R, E>
where
    E: StdError,
{
    receive_reply(result_rx)
        .await?
        .map_err(PluginError::Internal)
}

pub async fn send_message_receive_result<M, R, E>(
    message: impl Into<M>,
    message_tx: &MessageSender<M>,
    result_rx: ResultReceiver<R, E>,
) -> PluginResult<R, E>
where
    M: fmt::Debug,
    E: StdError,
{
    send_message(message, message_tx)?;
    receive_result(result_rx).await
}
