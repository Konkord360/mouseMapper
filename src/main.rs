mod config;
mod device;
mod engine;
mod tui;

use crate::config::Config;
use crate::device::reader::DeviceReader;
use crate::device::writer::DeviceWriter;
use crate::engine::mapper::EventMapper;
use crate::tui::app::{App, EngineCommand, EngineMessage};
use anyhow::{Context, Result};
use evdev::{EventType, InputEvent};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

fn main() -> Result<()> {
    // Initialize logging to a file (NOT stderr) so it doesn't corrupt the TUI.
    // Logs go to ~/.config/mouse-mapper/mouse-mapper.log
    init_file_logger();

    // Check for root access — record as a log warning, not eprintln (which corrupts TUI)
    if unsafe { libc::geteuid() } != 0 {
        log::warn!("mouse-mapper should be run as root (sudo) for /dev/input access");
    }

    // Load config
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
        Config::default()
    });

    // Create communication channels
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<EngineCommand>();
    let (msg_tx, msg_rx) = mpsc::unbounded_channel::<EngineMessage>();

    // Build the app
    let mut app = App::new(config);
    app.engine_cmd_tx = Some(cmd_tx);
    app.engine_msg_rx = Some(msg_rx);

    // Start the tokio runtime in a background thread for the engine
    let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    let _runtime_guard = runtime.enter();

    // Spawn the engine command handler
    let engine_msg_tx = msg_tx.clone();
    runtime.spawn(async move {
        engine_task(cmd_rx, engine_msg_tx).await;
    });

    // Run the TUI (blocks until quit)
    tui::run(app)?;

    // Cleanup: shutdown the runtime (will cancel all tasks including macros)
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));

    Ok(())
}

/// Initialize the logger to write to a file instead of stderr.
/// This prevents log output from corrupting the TUI which owns the terminal.
fn init_file_logger() {
    use std::fs;

    let log_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("mouse-mapper");
    let _ = fs::create_dir_all(&log_path);
    let log_file_path = log_path.join("mouse-mapper.log");

    // Open log file (truncate on each run to avoid unbounded growth)
    let log_file = match fs::File::create(&log_file_path) {
        Ok(f) => f,
        Err(_) => {
            // If we can't create a log file, just disable logging entirely
            // rather than corrupting the TUI
            log::set_max_level(log::LevelFilter::Off);
            return;
        }
    };
    let log_file = std::sync::Mutex::new(log_file);

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .target(env_logger::Target::Pipe(Box::new(LogWriter(log_file))))
        .init();
}

/// A simple Write adapter that forwards to a Mutex<File>.
struct LogWriter(std::sync::Mutex<std::fs::File>);

impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut f) = self.0.lock() {
            f.write(buf)
        } else {
            Ok(buf.len()) // Silently discard if lock is poisoned
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Ok(mut f) = self.0.lock() {
            f.flush()
        } else {
            Ok(())
        }
    }
}

/// Background task that handles engine commands and runs the event processing loop
async fn engine_task(
    mut cmd_rx: mpsc::UnboundedReceiver<EngineCommand>,
    msg_tx: mpsc::UnboundedSender<EngineMessage>,
) {
    let mut active_engine: Option<tokio::task::JoinHandle<()>> = None;
    let mut cancel_tx: Option<tokio::sync::watch::Sender<bool>> = None;

    loop {
        match cmd_rx.recv().await {
            Some(EngineCommand::Start(device_path)) => {
                // Stop any existing engine
                if let Some(tx) = cancel_tx.take() {
                    let _ = tx.send(true);
                }
                if let Some(handle) = active_engine.take() {
                    handle.abort();
                }

                let (new_cancel_tx, new_cancel_rx) = tokio::sync::watch::channel(false);
                cancel_tx = Some(new_cancel_tx);

                let msg_tx_clone = msg_tx.clone();
                let path = device_path.clone();

                active_engine = Some(tokio::spawn(async move {
                    match run_engine(&path, msg_tx_clone.clone(), new_cancel_rx).await {
                        Ok(()) => {
                            // Engine exited cleanly (e.g. device disconnected, channel closed)
                            let _ = msg_tx_clone
                                .send(EngineMessage::Error("Engine stopped unexpectedly".into()));
                        }
                        Err(e) => {
                            let _ = msg_tx_clone
                                .send(EngineMessage::Error(format!("{:#}", e)));
                        }
                    }
                }));

                let _ = msg_tx.send(EngineMessage::StatusUpdate(format!(
                    "Engine started on {}",
                    device_path
                )));
            }

            Some(EngineCommand::Stop) => {
                if let Some(tx) = cancel_tx.take() {
                    let _ = tx.send(true);
                }
                if let Some(handle) = active_engine.take() {
                    handle.abort();
                }
                let _ = msg_tx.send(EngineMessage::StatusUpdate("Engine stopped".into()));
            }

            Some(EngineCommand::ReloadConfig) => {
                let _ = msg_tx.send(EngineMessage::StatusUpdate(
                    "Config reload requested (restart engine to apply)".into(),
                ));
            }

            Some(EngineCommand::Shutdown) | None => {
                if let Some(tx) = cancel_tx.take() {
                    let _ = tx.send(true);
                }
                if let Some(handle) = active_engine.take() {
                    handle.abort();
                }
                break;
            }
        }
    }
}

/// Run the actual event processing engine
async fn run_engine(
    device_path: &str,
    msg_tx: mpsc::UnboundedSender<EngineMessage>,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    // Open and grab the device
    let mut reader = DeviceReader::open(Path::new(device_path))?;

    // Create virtual device mirroring the source capabilities
    let writer = DeviceWriter::from_source(reader.device())?;
    let writer = Arc::new(Mutex::new(writer));

    // Load config for the mapper
    let config = Config::load().unwrap_or_default();
    let mut mapper = EventMapper::new(writer.clone());
    mapper.load_config(&config);

    // Grab the device (exclusive access)
    reader.grab()?;

    let _ = msg_tx.send(EngineMessage::StatusUpdate(format!(
        "Grabbed device: {}",
        reader.name()
    )));

    // Create channel for events from the reader
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<InputEvent>();

    // Spawn the blocking reader in a dedicated thread
    let reader_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = reader.read_loop(event_tx) {
            log::error!("Reader error: {}", e);
        }
        // reader is dropped here, releasing the grab
    });

    // Process events
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(input_event) => {
                        // Send to monitor (skip EV_SYN and EV_MSC noise)
                        if input_event.event_type() != EventType::SYNCHRONIZATION
                            && input_event.event_type() != EventType::MISC
                        {
                            let _ = msg_tx.send(event_to_message(&input_event));
                        }

                        // Process through mapper
                        match mapper.process_event(input_event) {
                            Ok(output_events) => {
                                if !output_events.is_empty() {
                                    if let Ok(mut w) = writer.lock() {
                                        if let Err(e) = w.emit(&output_events) {
                                            log::error!("Failed to emit events: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Mapper error: {}", e);
                            }
                        }
                    }
                    None => {
                        // Reader channel closed
                        break;
                    }
                }
            }
            _ = cancel_rx.changed() => {
                // Cancellation requested
                mapper.stop_all();
                break;
            }
        }
    }

    // The reader task will stop when event_rx is dropped (it detects send failure)
    reader_handle.abort();

    Ok(())
}

/// Convert an InputEvent to an EngineMessage for the monitor
fn event_to_message(event: &InputEvent) -> EngineMessage {
    let event_type = match event.event_type() {
        EventType::SYNCHRONIZATION => "EV_SYN".to_string(),
        EventType::KEY => "EV_KEY".to_string(),
        EventType::RELATIVE => "EV_REL".to_string(),
        EventType::ABSOLUTE => "EV_ABS".to_string(),
        EventType::MISC => "EV_MSC".to_string(),
        other => format!("EV_{}", other.0),
    };

    let code = match event.event_type() {
        EventType::KEY => format!("{:?}", evdev::KeyCode::new(event.code())),
        EventType::RELATIVE => format!("{:?}", evdev::RelativeAxisCode(event.code())),
        EventType::ABSOLUTE => format!("{:?}", evdev::AbsoluteAxisCode(event.code())),
        _ => format!("{}", event.code()),
    };

    let timestamp = {
        let ts = event.timestamp();
        let duration = ts.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        format!("{}.{:06}", duration.as_secs() % 1000, duration.subsec_micros())
    };

    EngineMessage::RawEvent {
        event_type,
        code,
        value: event.value(),
        timestamp,
    }
}
