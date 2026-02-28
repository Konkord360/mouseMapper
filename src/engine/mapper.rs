use crate::config::{BindingOutput, Config, MacroDef};
use crate::device::writer::DeviceWriter;
use crate::engine::macros::MacroEngine;
use anyhow::Result;
use evdev::{EventType, InputEvent, KeyCode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Resolve a key name string (e.g. "BTN_LEFT", "KEY_Q") to an evdev KeyCode.
pub fn parse_key_name(name: &str) -> Option<KeyCode> {
    // Try matching against known button/key names
    // This covers the most common ones. evdev KeyCode codes are u16.
    let name_upper = name.to_uppercase();

    // Mouse buttons
    match name_upper.as_str() {
        "BTN_LEFT" | "BTN_MOUSE" => return Some(KeyCode::BTN_LEFT),
        "BTN_RIGHT" => return Some(KeyCode::BTN_RIGHT),
        "BTN_MIDDLE" => return Some(KeyCode::BTN_MIDDLE),
        "BTN_SIDE" => return Some(KeyCode::BTN_SIDE),
        "BTN_EXTRA" => return Some(KeyCode::BTN_EXTRA),
        "BTN_FORWARD" => return Some(KeyCode::BTN_FORWARD),
        "BTN_BACK" => return Some(KeyCode::BTN_BACK),
        "BTN_TASK" => return Some(KeyCode::BTN_TASK),
        _ => {}
    }

    // Keyboard keys - try KEY_ prefix
    let with_prefix = if name_upper.starts_with("KEY_") {
        name_upper.clone()
    } else {
        format!("KEY_{}", name_upper)
    };

    // Common keyboard keys
    match with_prefix.as_str() {
        "KEY_ESC" => Some(KeyCode::KEY_ESC),
        "KEY_1" => Some(KeyCode::KEY_1),
        "KEY_2" => Some(KeyCode::KEY_2),
        "KEY_3" => Some(KeyCode::KEY_3),
        "KEY_4" => Some(KeyCode::KEY_4),
        "KEY_5" => Some(KeyCode::KEY_5),
        "KEY_6" => Some(KeyCode::KEY_6),
        "KEY_7" => Some(KeyCode::KEY_7),
        "KEY_8" => Some(KeyCode::KEY_8),
        "KEY_9" => Some(KeyCode::KEY_9),
        "KEY_0" => Some(KeyCode::KEY_0),
        "KEY_MINUS" => Some(KeyCode::KEY_MINUS),
        "KEY_EQUAL" => Some(KeyCode::KEY_EQUAL),
        "KEY_BACKSPACE" => Some(KeyCode::KEY_BACKSPACE),
        "KEY_TAB" => Some(KeyCode::KEY_TAB),
        "KEY_Q" => Some(KeyCode::KEY_Q),
        "KEY_W" => Some(KeyCode::KEY_W),
        "KEY_E" => Some(KeyCode::KEY_E),
        "KEY_R" => Some(KeyCode::KEY_R),
        "KEY_T" => Some(KeyCode::KEY_T),
        "KEY_Y" => Some(KeyCode::KEY_Y),
        "KEY_U" => Some(KeyCode::KEY_U),
        "KEY_I" => Some(KeyCode::KEY_I),
        "KEY_O" => Some(KeyCode::KEY_O),
        "KEY_P" => Some(KeyCode::KEY_P),
        "KEY_LEFTBRACE" => Some(KeyCode::KEY_LEFTBRACE),
        "KEY_RIGHTBRACE" => Some(KeyCode::KEY_RIGHTBRACE),
        "KEY_ENTER" => Some(KeyCode::KEY_ENTER),
        "KEY_LEFTCTRL" => Some(KeyCode::KEY_LEFTCTRL),
        "KEY_A" => Some(KeyCode::KEY_A),
        "KEY_S" => Some(KeyCode::KEY_S),
        "KEY_D" => Some(KeyCode::KEY_D),
        "KEY_F" => Some(KeyCode::KEY_F),
        "KEY_G" => Some(KeyCode::KEY_G),
        "KEY_H" => Some(KeyCode::KEY_H),
        "KEY_J" => Some(KeyCode::KEY_J),
        "KEY_K" => Some(KeyCode::KEY_K),
        "KEY_L" => Some(KeyCode::KEY_L),
        "KEY_SEMICOLON" => Some(KeyCode::KEY_SEMICOLON),
        "KEY_APOSTROPHE" => Some(KeyCode::KEY_APOSTROPHE),
        "KEY_GRAVE" => Some(KeyCode::KEY_GRAVE),
        "KEY_LEFTSHIFT" => Some(KeyCode::KEY_LEFTSHIFT),
        "KEY_BACKSLASH" => Some(KeyCode::KEY_BACKSLASH),
        "KEY_Z" => Some(KeyCode::KEY_Z),
        "KEY_X" => Some(KeyCode::KEY_X),
        "KEY_C" => Some(KeyCode::KEY_C),
        "KEY_V" => Some(KeyCode::KEY_V),
        "KEY_B" => Some(KeyCode::KEY_B),
        "KEY_N" => Some(KeyCode::KEY_N),
        "KEY_M" => Some(KeyCode::KEY_M),
        "KEY_COMMA" => Some(KeyCode::KEY_COMMA),
        "KEY_DOT" => Some(KeyCode::KEY_DOT),
        "KEY_SLASH" => Some(KeyCode::KEY_SLASH),
        "KEY_RIGHTSHIFT" => Some(KeyCode::KEY_RIGHTSHIFT),
        "KEY_LEFTALT" => Some(KeyCode::KEY_LEFTALT),
        "KEY_SPACE" => Some(KeyCode::KEY_SPACE),
        "KEY_CAPSLOCK" => Some(KeyCode::KEY_CAPSLOCK),
        "KEY_F1" => Some(KeyCode::KEY_F1),
        "KEY_F2" => Some(KeyCode::KEY_F2),
        "KEY_F3" => Some(KeyCode::KEY_F3),
        "KEY_F4" => Some(KeyCode::KEY_F4),
        "KEY_F5" => Some(KeyCode::KEY_F5),
        "KEY_F6" => Some(KeyCode::KEY_F6),
        "KEY_F7" => Some(KeyCode::KEY_F7),
        "KEY_F8" => Some(KeyCode::KEY_F8),
        "KEY_F9" => Some(KeyCode::KEY_F9),
        "KEY_F10" => Some(KeyCode::KEY_F10),
        "KEY_F11" => Some(KeyCode::KEY_F11),
        "KEY_F12" => Some(KeyCode::KEY_F12),
        "KEY_RIGHTCTRL" => Some(KeyCode::KEY_RIGHTCTRL),
        "KEY_RIGHTALT" => Some(KeyCode::KEY_RIGHTALT),
        "KEY_HOME" => Some(KeyCode::KEY_HOME),
        "KEY_UP" => Some(KeyCode::KEY_UP),
        "KEY_PAGEUP" => Some(KeyCode::KEY_PAGEUP),
        "KEY_LEFT" => Some(KeyCode::KEY_LEFT),
        "KEY_RIGHT" => Some(KeyCode::KEY_RIGHT),
        "KEY_END" => Some(KeyCode::KEY_END),
        "KEY_DOWN" => Some(KeyCode::KEY_DOWN),
        "KEY_PAGEDOWN" => Some(KeyCode::KEY_PAGEDOWN),
        "KEY_INSERT" => Some(KeyCode::KEY_INSERT),
        "KEY_DELETE" => Some(KeyCode::KEY_DELETE),
        _ => {
            // Try parsing as raw code number
            if let Ok(code) = name.parse::<u16>() {
                Some(KeyCode::new(code))
            } else {
                None
            }
        }
    }
}

/// Get the human-readable name for a KeyCode
pub fn key_name(key: KeyCode) -> String {
    format!("{:?}", key)
}

/// The event mapper: takes raw input events and produces output events,
/// handling remapping and macro triggers.
pub struct EventMapper {
    /// Binding map: input KeyCode -> output action
    bindings: HashMap<KeyCode, BindingOutput>,
    /// Macro definitions: macro name -> MacroDef
    macro_defs: HashMap<String, MacroDef>,
    /// Macro engine for handling active macros
    macro_engine: MacroEngine,
}

impl EventMapper {
    pub fn new(writer: Arc<Mutex<DeviceWriter>>) -> Self {
        Self {
            bindings: HashMap::new(),
            macro_defs: HashMap::new(),
            macro_engine: MacroEngine::new(writer),
        }
    }

    /// Update bindings from config
    pub fn load_config(&mut self, config: &Config) {
        self.bindings.clear();
        self.macro_defs.clear();

        let binding_map = config.build_binding_map();
        let macro_map = config.build_macro_map();

        for (key_name_str, output) in binding_map {
            if let Some(key) = parse_key_name(&key_name_str) {
                self.bindings.insert(key, output);
            } else {
                log::warn!("Unknown key name in binding: {}", key_name_str);
            }
        }

        self.macro_defs = macro_map;
        log::info!(
            "Loaded {} bindings, {} macros",
            self.bindings.len(),
            self.macro_defs.len()
        );
    }

    /// Process an input event. Returns events to emit (may be empty if handled by macro).
    pub fn process_event(&mut self, event: InputEvent) -> Result<Vec<InputEvent>> {
        // Only process key/button events for mapping
        if event.event_type() != EventType::KEY {
            // Pass through non-key events unchanged (mouse movement, scroll, sync, etc.)
            return Ok(vec![event]);
        }

        let key = KeyCode::new(event.code());
        let value = event.value(); // 0=release, 1=press, 2=repeat

        // Check if this key has a binding
        if let Some(binding) = self.bindings.get(&key).cloned() {
            match binding {
                BindingOutput::Key { key: ref key_name } => {
                    // Simple remap: translate to a different key
                    if let Some(target_key) = parse_key_name(key_name) {
                        let remapped = InputEvent::new(EventType::KEY.0, target_key.code(), value);
                        return Ok(vec![remapped]);
                    } else {
                        log::warn!("Unknown target key: {}", key_name);
                        return Ok(vec![event]);
                    }
                }
                BindingOutput::Macro { ref macro_name } => {
                    // Trigger macro
                    if let Some(macro_def) = self.macro_defs.get(macro_name).cloned() {
                        match value {
                            1 => {
                                // Button pressed - start macro
                                self.macro_engine.start_macro(key, &macro_def)?;
                                return Ok(vec![]); // Consume the event
                            }
                            0 => {
                                // Button released - stop macro (for hold-type)
                                self.macro_engine.stop_macro(key);
                                return Ok(vec![]); // Consume the event
                            }
                            _ => {
                                // Repeat events - consume them for macro-bound buttons
                                return Ok(vec![]);
                            }
                        }
                    } else {
                        log::warn!("Macro not found: {}", macro_name);
                        return Ok(vec![event]);
                    }
                }
            }
        }

        // No binding - pass through
        Ok(vec![event])
    }

    /// Stop all running macros (for clean shutdown)
    pub fn stop_all(&mut self) {
        self.macro_engine.stop_all();
    }
}
