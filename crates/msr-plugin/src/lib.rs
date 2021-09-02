use msr_core::audit::Activity;
use std::{fmt, future::Future, pin::Pin};
use tokio::sync::{broadcast, mpsc, oneshot};

// ------ -------
//  Plugin shape
// ------ -------

#[allow(missing_debug_implementations)]
pub struct Plugin<M, P, E> {
    pub ports: PluginPorts<M, P, E>,
    pub message_loop: MessageLoop,
}

pub type MessageLoop = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[allow(missing_debug_implementations)]
pub struct PluginPorts<M, P, E> {
    pub message_tx: MessageSender<M>,
    pub event_subscriber: EventSubscriber<P, E>,
}

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

pub type BroadcastSender<T> = broadcast::Sender<T>;
pub type BroadcastReceiver<T> = broadcast::Receiver<T>;

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
