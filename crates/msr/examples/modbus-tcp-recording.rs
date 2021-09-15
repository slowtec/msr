use anyhow::Result;
use msr_plugin::{MessageLoop, Plugin};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, UnboundedSender},
    },
    task::JoinHandle,
};

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    log::info!("Starting Modbus TCP recording example");

    let (shutdown_tx, shutdown_rx) = broadcast::channel(100);

    log::info!("Spawning tasks");
    let main_task = spawn_tasks(shutdown_rx)?;

    loop {
        // TODO: The select macro is actually not needed here an only
        // required for handling more complex use cases.
        tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::debug!("Received CTRL+C");
            log::info!("Terminating Modbus TCP recording example");
            shutdown_tx.send(())?;
            break;
        }
        }
    }

    log::info!("Awaiting termination of tasks");
    main_task.await?;

    Ok(())
}

async fn run_mediator(
    modbus_tx: UnboundedSender<ModbusMessage>,
    recorder_tx: UnboundedSender<RecorderMessage>,
    mut modbus_event_rx: broadcast::Receiver<ModbusEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    log::info!("Starting mediator task");
    loop {
        tokio::select! {
            Ok(ev) = modbus_event_rx.recv() => {
            match ev {
                ModbusEvent::Data(x) => {
                    if recorder_tx.send(RecorderMessage::Record(x)).is_err() {
                        log::warn!("The recorder plugin message channel was closed");
                    }
                }
            }
            }
            _ = shutdown_rx.recv() => {
                if modbus_tx.send(ModbusMessage::Shutdown).is_err() {
                    log::warn!("The modbus plugin message channel was closed");
                }
                if recorder_tx.send(RecorderMessage::Shutdown).is_err() {
                    log::warn!("The recorder plugin message channel was closed");
                }
                break;
            }
            else => {
                break;
            }
        }
    }
    log::info!("Exiting mediator task");
}

fn spawn_mediator(
    modbus_plugin: &ModbusPlugin,
    recorder_plugin: &RecorderPlugin,
    shutdown_rx: broadcast::Receiver<()>,
) -> JoinHandle<()> {
    let modbus_tx = modbus_plugin.message_sender();
    let recorder_tx = recorder_plugin.message_sender();
    let modbus_event_rx = modbus_plugin.subscribe_events();

    // Spawn event mediators in any order before starting the plugins
    // ...they should do nothing until events from plugins are received
    tokio::spawn(run_mediator(
        modbus_tx,
        recorder_tx,
        modbus_event_rx,
        shutdown_rx,
    ))
}

fn spawn_tasks(shutdown_rx: broadcast::Receiver<()>) -> Result<JoinHandle<()>> {
    log::info!("Setting up plugins");
    let modbus_plugin = ModbusPlugin::setup()?;
    let recorder_plugin = RecorderPlugin::setup()?;

    log::info!("Spawning mediator task");
    // The mediator is passive and should be spawned before any plugin
    // is running. Otherwise some initial event messages from plugins
    // might not be received and get lost.
    let mediator_task = spawn_mediator(&modbus_plugin, &recorder_plugin, shutdown_rx);

    log::info!("Spawning plugin tasks");
    let modbus_plugin_task = tokio::spawn(modbus_plugin.run());
    let recorder_plugin_task = tokio::spawn(recorder_plugin.run());

    let main_task = tokio::spawn(async move {
        log::info!("Starting main task");
        match tokio::join!(mediator_task, modbus_plugin_task, recorder_plugin_task) {
            (Ok(_), Ok(_), Ok(_)) => {
                log::info!("All worker tasks terminated");
            }
            (mediator_task, modbus_plugin_task, recorder_plugin_task) => {
                if let Err(err) = mediator_task {
                    log::error!("Failed to join mediator task: {}", err);
                }
                if let Err(err) = modbus_plugin_task {
                    log::error!("Failed to join modbus plugin task: {}", err);
                }
                if let Err(err) = recorder_plugin_task {
                    log::error!("Failed to join recorder plugin task: {}", err);
                }
            }
        }
        log::info!("Terminating main task");
    });

    Ok(main_task)
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

        let event_tx = broadcast_tx.clone();
        let message_loop = Box::pin(async move {
            log::info!("Entering modbus plugin message loop");
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut cnt = 0;
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let event = ModbusEvent::Data(cnt);
                        if let Err(event) = event_tx.send(event) {
                            log::debug!("No subscribers, dropping event: {:?}", event);
                        }
                        cnt += 1;
                    }
                    next_msg = message_rx.recv() => {
                        if let Some(msg) = next_msg {
                        log::debug!("Received message: {:?}", msg);
                        match msg {
                            ModbusMessage::Shutdown => {
                              break;
                            }
                        }
                    } else {
                        log::info!("All message senders have been dropped");
                        break;
                    }
                    }
                }
            }
            log::info!("Exiting modbus plugin message loop");
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
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event> {
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
            log::info!("Entering recorder plugin message loop");
            loop {
                match message_rx.recv().await {
                    Some(msg) => {
                        log::debug!("Received message: {:?}", msg);
                        match msg {
                            RecorderMessage::Record(data) => {
                                log::debug!("Recording data: {}", data);
                            }
                            RecorderMessage::Shutdown => {
                                break;
                            }
                        }
                    }
                    None => {
                        log::info!("All message senders have been dropped");
                        break;
                    }
                }
            }
            log::info!("Exiting recoder plugin message loop");
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
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event> {
        self.broadcast_tx.subscribe()
    }
    fn run(self) -> MessageLoop {
        self.message_loop
    }
}
