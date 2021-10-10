use msr_core::audit::Activity;
use std::{error::Error as StdError, fmt, future::Future, pin::Pin};
use thiserror::Error;
use tokio::sync::{
    broadcast,
    mpsc::{self, error::SendError},
    oneshot,
};

// ------ -------
//  Plugin shape
// ------ -------

pub trait Plugin {
    type Message;
    type Event;
    fn message_sender(&self) -> mpsc::UnboundedSender<Self::Message>;
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event>;
    fn run(self) -> MessageLoop;
}

#[allow(missing_debug_implementations)]
pub struct PluginContainer<M, P, E> {
    pub ports: PluginPorts<M, P, E>,
    pub message_loop: MessageLoop,
}

impl<M, P, E> Plugin for PluginContainer<M, P, E> {
    type Message = M;
    type Event = PublishedEvent<P, E>;
    fn message_sender(&self) -> mpsc::UnboundedSender<Self::Message> {
        self.ports.message_tx.clone()
    }
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event> {
        self.ports.event_subscriber.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}

pub type MessageLoop = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[allow(missing_debug_implementations)]
pub struct PluginPorts<M, P, E> {
    pub message_tx: MessageSender<M>,
    pub event_subscriber: EventSubscriber<P, E>,
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
type MessageSender<T> = mpsc::UnboundedSender<T>;
type MessageReceiver<T> = mpsc::UnboundedReceiver<T>;

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

#[derive(Debug, Clone)]
pub struct PublishedEvent<P, T> {
    pub published: Activity<P>,
    pub payload: T,
}

pub type EventSender<P, T> = broadcast::Sender<PublishedEvent<P, T>>;
pub type EventReceiver<P, T> = broadcast::Receiver<PublishedEvent<P, T>>;
pub type EventSubscriber<P, T> = BroadcastSubscriber<PublishedEvent<P, T>>;

pub fn event_channel<P, T>(channel_capacity: usize) -> (EventSender<P, T>, EventSubscriber<P, T>)
where
    P: Clone,
    T: Clone,
{
    broadcast_channel(channel_capacity)
}

#[derive(Debug, Clone)]
pub struct EventPubSub<P, E> {
    publisher: P,
    event_tx: EventSender<P, E>,
}

impl<P, T> EventPubSub<P, T>
where
    P: fmt::Debug + Clone,
    T: fmt::Debug + Clone,
{
    pub fn new(publisher: impl Into<P>, channel_capacity: usize) -> (Self, EventSubscriber<P, T>) {
        let (event_tx, event_subscriber) = event_channel(channel_capacity);
        (
            Self {
                event_tx,
                publisher: publisher.into(),
            },
            event_subscriber,
        )
    }

    pub fn publish_event(&self, payload: T) {
        let publisher = self.publisher.clone();
        let published = Activity::now(publisher);
        let event = PublishedEvent { published, payload };
        self.dispatch_event(event);
    }
}

pub trait EventDispatcher<E> {
    fn dispatch_event(&self, event: E);
}

impl<P, T> EventDispatcher<PublishedEvent<P, T>> for EventPubSub<P, T>
where
    P: fmt::Debug + Clone,
    T: fmt::Debug + Clone,
{
    fn dispatch_event(&self, event: PublishedEvent<P, T>) {
        if let Err(event) = self.event_tx.send(event) {
            // Ignore all send errors that are expected if no subscribers
            // are connected.
            log::debug!("No subscribers for published event {:?}", event);
        }
    }
}

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
