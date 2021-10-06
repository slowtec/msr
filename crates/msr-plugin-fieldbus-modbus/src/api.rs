use std::net::SocketAddr;

#[derive(Debug)]
pub enum Message {
    Connect(ConnCfg),
    Shutdown,
}

#[derive(Debug)]
pub enum ConnCfg {
    Tcp(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connecting,
    Connected,
    ConnectingError(String),
}
