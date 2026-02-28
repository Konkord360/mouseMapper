use crate::config::{Binding, BindingOutput, Config, MacroAction, MacroDef, MacroType};
use crate::device::scanner::{self, DeviceInfo};
use std::time::Instant;
use tokio::sync::mpsc;

/// Which tab is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Devices,
    Bindings,
    Macros,
    Monitor,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Devices, Tab::Bindings, Tab::Macros, Tab::Monitor]
    }

    pub fn title(&self) -> &str {
        match self {
            Tab::Devices => "Devices",
            Tab::Bindings => "Bindings",
            Tab::Macros => "Macros",
            Tab::Monitor => "Monitor",
        }
    }

    pub fn next(&self) -> Tab {
        match self {
            Tab::Devices => Tab::Bindings,
            Tab::Bindings => Tab::Macros,
            Tab::Macros => Tab::Monitor,
            Tab::Monitor => Tab::Devices,
        }
    }

    pub fn prev(&self) -> Tab {
        match self {
            Tab::Devices => Tab::Monitor,
            Tab::Bindings => Tab::Devices,
            Tab::Macros => Tab::Bindings,
            Tab::Monitor => Tab::Macros,
        }
    }
}

/// Input mode for the TUI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// Normal navigation
    Normal,
    /// Editing a text field
    Editing(String),
    /// Waiting for a key press to capture (for binding input/output)
    Capturing { field: CaptureField },
    /// Confirming an action
    Confirming(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureField {
    BindingInput,
    BindingOutput,
}

/// Messages from the engine to the TUI
#[derive(Debug, Clone)]
pub enum EngineMessage {
    /// A raw input event was received (for the monitor tab)
    RawEvent {
        event_type: String,
        code: String,
        value: i32,
        timestamp: String,
    },
    /// Engine status changed
    StatusUpdate(String),
    /// Engine encountered an error
    Error(String),
}

/// Commands from the TUI to the engine
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// Start the engine with the given device path
    Start(String),
    /// Stop the engine
    Stop,
    /// Reload config
    ReloadConfig,
    /// Shutdown everything
    Shutdown,
}

/// Application state
pub struct App {
    pub config: Config,
    pub current_tab: Tab,
    pub input_mode: InputMode,
    pub should_quit: bool,

    // Device tab state
    pub devices: Vec<DeviceInfo>,
    pub device_list_index: usize,
    pub selected_device: Option<DeviceInfo>,
    pub engine_running: bool,

    // Bindings tab state
    pub binding_list_index: usize,
    pub editing_binding: Option<EditingBinding>,

    // Macros tab state
    pub macro_list_index: usize,
    pub editing_macro: Option<EditingMacro>,

    // Monitor tab state
    pub monitor_events: Vec<EngineMessage>,
    pub monitor_paused: bool,
    pub monitor_max_events: usize,

    // Communication channels
    pub engine_cmd_tx: Option<mpsc::UnboundedSender<EngineCommand>>,
    pub engine_msg_rx: Option<mpsc::UnboundedReceiver<EngineMessage>>,

    /// True while waiting for a mouse button press to capture via the engine event stream
    pub capturing: bool,

    // Status bar
    pub status_message: String,
    pub status_time: Instant,
}

/// State for editing a binding
#[derive(Debug, Clone)]
pub struct EditingBinding {
    pub index: Option<usize>, // None = new binding
    pub input: String,
    pub output_type: BindingOutputType,
    pub output_value: String,
    pub field_index: usize,        // 0=input, 1=output_type, 2=output_value
    pub macro_select_index: usize, // index in the macro list when output_type is Macro
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingOutputType {
    Key,
    Macro,
}

/// State for editing a macro
#[derive(Debug, Clone)]
pub struct EditingMacro {
    pub index: Option<usize>,
    pub name: String,
    pub macro_type: MacroType,
    pub actions: Vec<MacroAction>,
    pub interval_ms: String,
    pub jitter_ms: String,
    pub field_index: usize, // which field is focused
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            current_tab: Tab::Devices,
            input_mode: InputMode::Normal,
            should_quit: false,

            devices: Vec::new(),
            device_list_index: 0,
            selected_device: None,
            engine_running: false,

            binding_list_index: 0,
            editing_binding: None,

            macro_list_index: 0,
            editing_macro: None,

            monitor_events: Vec::new(),
            monitor_paused: false,
            monitor_max_events: 500,

            engine_cmd_tx: None,
            engine_msg_rx: None,

            capturing: false,

            status_message: String::from("Press ? for help"),
            status_time: Instant::now(),
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_time = Instant::now();
    }

    /// Refresh the device list
    pub fn refresh_devices(&mut self) {
        match scanner::scan_devices() {
            Ok(devices) => {
                self.devices = devices;
                self.set_status(format!("Found {} devices", self.devices.len()));
            }
            Err(e) => {
                self.set_status(format!("Error scanning devices: {}", e));
            }
        }
    }

    /// Get bindings for the active profile
    pub fn current_bindings(&self) -> &[Binding] {
        self.config
            .active_profile()
            .map(|p| p.bindings.as_slice())
            .unwrap_or(&[])
    }

    /// Get macros for the active profile
    pub fn current_macros(&self) -> &[MacroDef] {
        self.config
            .active_profile()
            .map(|p| p.macros.as_slice())
            .unwrap_or(&[])
    }

    /// Select the device at the current index and update config
    pub fn select_current_device(&mut self) {
        if let Some(device) = self.devices.get(self.device_list_index) {
            self.selected_device = Some(device.clone());
            self.config.device.name = Some(device.name.clone());
            self.config.device.path = Some(device.path.to_string_lossy().to_string());
            self.config.device.vendor_id = Some(device.vendor_id);
            self.config.device.product_id = Some(device.product_id);
            self.set_status(format!("Selected: {}", device.name));
        }
    }

    /// Toggle the engine (start/stop)
    pub fn toggle_engine(&mut self) {
        if self.engine_running {
            self.send_engine_command(EngineCommand::Stop);
            self.engine_running = false;
            self.set_status("Engine stopped");
        } else if let Some(ref device) = self.selected_device {
            let path = device.path.to_string_lossy().to_string();
            self.send_engine_command(EngineCommand::Start(path));
            self.engine_running = true;
            self.set_status("Engine started");
        } else {
            self.set_status("No device selected! Select a device first.");
        }
    }

    fn send_engine_command(&self, cmd: EngineCommand) {
        if let Some(ref tx) = self.engine_cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    /// Process incoming engine messages.
    /// Caps the number of messages processed per tick to prevent the UI from freezing
    /// when the engine produces a burst of events (e.g. rapid mouse movement).
    /// Also intercepts EV_KEY press events for button capture when in capture mode.
    pub fn poll_engine_messages(&mut self) {
        let mut rx = match self.engine_msg_rx.take() {
            Some(rx) => rx,
            None => return,
        };

        const MAX_MESSAGES_PER_TICK: usize = 200;
        let mut processed = 0;

        while processed < MAX_MESSAGES_PER_TICK {
            match rx.try_recv() {
                Ok(msg) => {
                    processed += 1;
                    match &msg {
                        EngineMessage::StatusUpdate(s) => {
                            self.set_status(s.clone());
                        }
                        EngineMessage::Error(e) => {
                            self.set_status(format!("ERROR: {}", e));
                            self.engine_running = false;
                        }
                        EngineMessage::RawEvent {
                            event_type,
                            code,
                            value,
                            ..
                        } => {
                            // If we're in capture mode and this is a button press,
                            // intercept it for capture instead of adding to monitor
                            if self.capturing && event_type == "EV_KEY" && *value == 1 {
                                let captured = code.clone();
                                // Apply the captured key name to the appropriate field
                                match &self.input_mode {
                                    InputMode::Capturing { field } => match field {
                                        CaptureField::BindingInput => {
                                            if let Some(ref mut editing) = self.editing_binding {
                                                editing.input = captured.clone();
                                            }
                                        }
                                        CaptureField::BindingOutput => {
                                            if let Some(ref mut editing) = self.editing_binding {
                                                editing.output_value = captured.clone();
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                                self.capturing = false;
                                self.input_mode = InputMode::Editing(String::new());
                                self.set_status(format!("Captured: {}", captured));
                                // Don't add this event to monitor — it was consumed by capture
                                continue;
                            }

                            if !self.monitor_paused {
                                self.monitor_events.push(msg.clone());
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Trim monitor events to max capacity (do it once at the end, not per message)
        if self.monitor_events.len() > self.monitor_max_events {
            let drain_count = self.monitor_events.len() - self.monitor_max_events;
            self.monitor_events.drain(..drain_count);
        }

        self.engine_msg_rx = Some(rx);
    }

    // === Binding editing ===

    pub fn start_new_binding(&mut self) {
        self.editing_binding = Some(EditingBinding {
            index: None,
            input: String::new(),
            output_type: BindingOutputType::Key,
            output_value: String::new(),
            field_index: 0,
            macro_select_index: 0,
        });
        self.input_mode = InputMode::Editing(String::new());
    }

    pub fn start_edit_binding(&mut self) {
        let bindings = self.current_bindings().to_vec();
        if let Some(binding) = bindings.get(self.binding_list_index) {
            let (output_type, output_value) = match &binding.output {
                BindingOutput::Key { key } => (BindingOutputType::Key, key.clone()),
                BindingOutput::Macro { macro_name } => {
                    (BindingOutputType::Macro, macro_name.clone())
                }
            };
            // If editing a macro binding, try to find the index of the selected macro
            let macro_select_index = if output_type == BindingOutputType::Macro {
                self.current_macros()
                    .iter()
                    .position(|m| m.name == output_value)
                    .unwrap_or(0)
            } else {
                0
            };
            self.editing_binding = Some(EditingBinding {
                index: Some(self.binding_list_index),
                input: binding.input.clone(),
                output_type,
                output_value,
                field_index: 0,
                macro_select_index,
            });
            self.input_mode = InputMode::Editing(String::new());
        }
    }

    pub fn save_editing_binding(&mut self) {
        if let Some(ref editing) = self.editing_binding.clone() {
            let output = match editing.output_type {
                BindingOutputType::Key => BindingOutput::Key {
                    key: editing.output_value.clone(),
                },
                BindingOutputType::Macro => BindingOutput::Macro {
                    macro_name: editing.output_value.clone(),
                },
            };
            let binding = Binding {
                input: editing.input.clone(),
                output,
            };

            if let Some(profile) = self.config.active_profile_mut() {
                if let Some(idx) = editing.index {
                    if idx < profile.bindings.len() {
                        profile.bindings[idx] = binding;
                    }
                } else {
                    profile.bindings.push(binding);
                }
            }

            self.editing_binding = None;
            self.input_mode = InputMode::Normal;
            self.set_status("Binding saved");
        }
    }

    pub fn delete_current_binding(&mut self) {
        let idx = self.binding_list_index;
        if let Some(profile) = self.config.active_profile_mut() {
            if idx < profile.bindings.len() {
                profile.bindings.remove(idx);
                if self.binding_list_index > 0 && self.binding_list_index >= profile.bindings.len()
                {
                    self.binding_list_index = profile.bindings.len().saturating_sub(1);
                }
            }
        }
        self.set_status("Binding deleted");
    }

    // === Macro editing ===

    pub fn start_new_macro(&mut self) {
        self.editing_macro = Some(EditingMacro {
            index: None,
            name: String::new(),
            macro_type: MacroType::RepeatOnHold,
            actions: vec![MacroAction::Click("BTN_LEFT".to_string())],
            interval_ms: "50".to_string(),
            jitter_ms: "10".to_string(),
            field_index: 0,
        });
        self.input_mode = InputMode::Editing(String::new());
    }

    pub fn start_edit_macro(&mut self) {
        let macros = self.current_macros().to_vec();
        if let Some(macro_def) = macros.get(self.macro_list_index) {
            self.editing_macro = Some(EditingMacro {
                index: Some(self.macro_list_index),
                name: macro_def.name.clone(),
                macro_type: macro_def.macro_type.clone(),
                actions: macro_def.actions.clone(),
                interval_ms: macro_def.interval_ms.to_string(),
                jitter_ms: macro_def.jitter_ms.to_string(),
                field_index: 0,
            });
            self.input_mode = InputMode::Editing(String::new());
        }
    }

    pub fn save_editing_macro(&mut self) {
        if let Some(ref editing) = self.editing_macro.clone() {
            let interval_ms = editing.interval_ms.parse().unwrap_or(50);
            let jitter_ms = editing.jitter_ms.parse().unwrap_or(0);
            let macro_def = MacroDef {
                name: editing.name.clone(),
                macro_type: editing.macro_type.clone(),
                actions: editing.actions.clone(),
                interval_ms,
                initial_delay_ms: 0,
                jitter_ms,
            };

            if let Some(profile) = self.config.active_profile_mut() {
                if let Some(idx) = editing.index {
                    if idx < profile.macros.len() {
                        profile.macros[idx] = macro_def;
                    }
                } else {
                    profile.macros.push(macro_def);
                }
            }

            self.editing_macro = None;
            self.input_mode = InputMode::Normal;
            self.set_status("Macro saved");
        }
    }

    pub fn delete_current_macro(&mut self) {
        let idx = self.macro_list_index;
        if let Some(profile) = self.config.active_profile_mut() {
            if idx < profile.macros.len() {
                profile.macros.remove(idx);
                if self.macro_list_index > 0 && self.macro_list_index >= profile.macros.len() {
                    self.macro_list_index = profile.macros.len().saturating_sub(1);
                }
            }
        }
        self.set_status("Macro deleted");
    }

    /// Save config to disk
    pub fn save_config(&mut self) {
        match self.config.save() {
            Ok(()) => self.set_status("Config saved"),
            Err(e) => self.set_status(format!("Failed to save config: {}", e)),
        }

        // Also tell the engine to reload
        self.send_engine_command(EngineCommand::ReloadConfig);
    }

    /// Start capturing a mouse button press via the engine's event stream.
    /// The engine must be running — it reads events from the grabbed device and
    /// forwards them as `EngineMessage::RawEvent`. `poll_engine_messages()` will
    /// intercept the first EV_KEY press while `self.capturing` is true.
    pub fn start_capture(&mut self, field: CaptureField) {
        if !self.engine_running {
            self.set_status("Start the engine first to capture buttons!");
            return;
        }

        let msg = match &field {
            CaptureField::BindingInput => "Press a mouse button to capture... (Esc to cancel)",
            CaptureField::BindingOutput => {
                "Press a key or mouse button to capture... (Esc to cancel)"
            }
        };

        self.capturing = true;
        self.input_mode = InputMode::Capturing { field };
        self.set_status(msg);
    }

    /// Get the list of macro names from the active profile
    pub fn macro_names(&self) -> Vec<String> {
        self.current_macros()
            .iter()
            .map(|m| m.name.clone())
            .collect()
    }
}
