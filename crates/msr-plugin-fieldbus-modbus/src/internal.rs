use crate::api::{ConnCfg, Event, Message};
use msr_plugin::MessageLoop;
use std::net::SocketAddr;
use tokio::sync::{broadcast, mpsc};

pub struct Context {
    conn: Conn,
    event_tx: broadcast::Sender<Event>,
}

impl Context {
    fn new(event_tx: broadcast::Sender<Event>) -> Self {
        Self {
            conn: Conn::default(),
            event_tx,
        }
    }
    fn send_ev(&self, ev: Event) {
        if let Err(ev) = self.event_tx.send(ev) {
            log::debug!("No subscribers, dropping event: {:?}", ev);
        }
    }
    async fn connect_tcp(&mut self, addr: SocketAddr) {
        match self.conn {
            Conn::Disconnected => {
                self.conn = Conn::Connecting;
                self.send_ev(Event::Connecting);
                match tokio_modbus::client::tcp::connect(addr).await {
                    Ok(mb_ctx) => {
                        self.conn = Conn::Connected(mb_ctx);
                        self.send_ev(Event::Connected);
                    }
                    Err(err) => {
                        self.send_ev(Event::ConnectingError(err.to_string()));
                    }
                }
            }
            Conn::Connecting => {
                self.send_ev(Event::ConnectingError("Already connecting".to_string()));
            }
            Conn::Connected(_) => {
                self.send_ev(Event::ConnectingError("Already connected".to_string()));
            }
        }
    }
}

enum Conn {
    Disconnected,
    Connecting,
    Connected(tokio_modbus::client::Context),
}

impl Default for Conn {
    fn default() -> Self {
        Self::Disconnected
    }
}

pub fn create_message_loop(
    event_tx: broadcast::Sender<Event>,
) -> (MessageLoop, mpsc::UnboundedSender<Message>) {
    let mut ctx = Context::new(event_tx);
    let (message_tx, mut message_rx) = mpsc::unbounded_channel();
    let message_loop = async move {
        log::debug!("Entering modbus plugin message loop");
        loop {
            tokio::select! {
                next_msg = message_rx.recv() => {
                    if let Some(msg) = next_msg {
                    log::debug!("Received message: {:?}", msg);
                    match msg {
                        Message::Connect(cfg) => {
                            match cfg {
                              ConnCfg::Tcp(addr) => {
                                ctx.connect_tcp(addr).await;
                              }
                            }
                        }
                        Message::Shutdown => {
                          break;
                        }
                    }
                } else {
                    log::debug!("All message senders have been dropped");
                    break;
                }
                }
            }
        }
        log::debug!("Exiting modbus plugin message loop");
    };
    (Box::pin(message_loop), message_tx)
}
