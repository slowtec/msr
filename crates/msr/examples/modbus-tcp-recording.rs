use anyhow::Result;
use msr_plugin::{MessageLoop, Plugin};
use tokio::sync::{broadcast, mpsc};

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    log::info!("Starting Modbus TCP recording example");

    let (shutdown_tx, shutdown_rx) = broadcast::channel(100);

    let plugin_tasks = spawn_plugins(shutdown_rx);

    tokio::select! {
      _ = tokio::signal::ctrl_c() => {
        log::debug!("received CTRL+C");
        log::info!("Terminating Modbus TCP recording example");
        shutdown_tx.send(())?;
      }
      _ = plugin_tasks => {
          // plugins terminated
      }
    }
    Ok(())
}

async fn spawn_plugins(mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
    log::info!("Spawning plugins");

    let modbus_plugin = ModbusPlugin::setup()?;
    let recorder_plugin = RecorderPlugin::setup()?;

    let mut event_rx = modbus_plugin.subscribe();
    let recorder_tx = recorder_plugin.message_sender();
    let modbus_tx = modbus_plugin.message_sender();

    // Spawn event mediators in any order before starting the plugins
    // ...they should do nothing until events from plugins are received
    let mediator = tokio::spawn(async move {
        loop {
            tokio::select! {
              Ok(ev) = event_rx.recv() => {
                match ev {
                    ModbusEvent::Data(x) => {
                        if let Err(_) = recorder_tx.send(RecorderMessage::Record(x)) {
                            log::warn!("The recorder plugin message channel was closed");
                            return;
                        }
                    }
                }
              }
              _ = shutdown_rx.recv() => {
                  if let Err(_) = modbus_tx.send(ModbusMessage::Shutdown) {
                      log::warn!("The modbus plugin message channel was closed");
                  }
                  if let Err(_) = recorder_tx.send(RecorderMessage::Shutdown) {
                      log::warn!("The recorder plugin message channel was closed");
                  }
                  break;
              }
            }
        }
        log::info!("Terminating the mediator");
    });

    let recorder = tokio::spawn(recorder_plugin.run());
    let modbus = tokio::spawn(modbus_plugin.run());

    for task in [mediator, recorder, modbus] {
        task.await?;
    }
    log::info!("All plugin tasks terminated");
    Ok(())
}

// ------    ------ //
//  Modbus Plugin   //
// ------    ------ //

struct ModbusPlugin {
    message_loop: MessageLoop,
    message_tx: mpsc::UnboundedSender<ModbusMessage>,
    broadcast_tx: broadcast::Sender<ModbusEvent>,
}

#[derive(Debug)]
pub enum ModbusMessage {
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum ModbusEvent {
    Data(usize),
}

impl ModbusPlugin {
    pub fn setup() -> Result<Self> {
        let (message_tx, mut message_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(100);

        let ev_tx = broadcast_tx.clone();
        let message_loop = Box::pin(async move {
            log::info!("Start modbus plugin message looop");
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            let mut cnt = 0;
            loop {
                tokio::select! {
                  _ = interval.tick() => {
                    if let Err(_) = ev_tx.send(ModbusEvent::Data(cnt)) {
                      log::debug!("Could not send event: no modbus event listener subscribed");
                    }
                    cnt += 1;
                  }
                  Some(msg) = message_rx.recv() => {
                      log::debug!("Received message: {:?}", msg);
                      match msg {
                          ModbusMessage::Shutdown => {
                            break;
                          }
                      }
                  }
                }
            }
            log::info!("Terminating the modbus plugin message loop");
        });
        Ok(Self {
            message_tx,
            message_loop,
            broadcast_tx,
        })
    }
}

impl Plugin for ModbusPlugin {
    type Message = ModbusMessage;
    type Event = ModbusEvent;
    fn message_sender(&self) -> mpsc::UnboundedSender<Self::Message> {
        self.message_tx.clone()
    }
    fn subscribe(&self) -> broadcast::Receiver<Self::Event> {
        self.broadcast_tx.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}

// ------    ------ //
//  Recorder Plugin //
// ------    ------ //

struct RecorderPlugin {
    message_loop: MessageLoop,
    message_tx: mpsc::UnboundedSender<RecorderMessage>,
    broadcast_tx: broadcast::Sender<RecorderEvent>,
}

#[derive(Debug)]
pub enum RecorderMessage {
    Record(usize),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum RecorderEvent {}

impl RecorderPlugin {
    pub fn setup() -> Result<Self> {
        let (message_tx, mut message_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(100);
        let message_loop = Box::pin(async move {
            log::info!("Start recorder plugin message loop");
            loop {
                tokio::select! {
                  Some(msg) = message_rx.recv() => {
                      match msg {
                          RecorderMessage::Record(data) => {
                              log::debug!("Recording data: {}", data);
                          }
                          RecorderMessage::Shutdown => {
                            break;
                          }
                      }
                  }
                }
            }
            log::info!("Terminating the recoder plugin message loop");
        });
        Ok(Self {
            message_tx,
            message_loop,
            broadcast_tx,
        })
    }
}

impl Plugin for RecorderPlugin {
    type Message = RecorderMessage;
    type Event = RecorderEvent;
    fn message_sender(&self) -> mpsc::UnboundedSender<Self::Message> {
        self.message_tx.clone()
    }
    fn subscribe(&self) -> broadcast::Receiver<Self::Event> {
        self.broadcast_tx.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}
