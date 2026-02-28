use crate::config::{MacroAction, MacroDef, MacroType};
use crate::device::writer::DeviceWriter;
use crate::engine::mapper::parse_key_name;
use anyhow::Result;
use evdev::KeyCode;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Manages running macro instances
pub struct MacroEngine {
    writer: Arc<Mutex<DeviceWriter>>,
    /// Active macros: trigger key -> cancel sender
    active: HashMap<KeyCode, watch::Sender<bool>>,
    /// Toggle state for toggle macros
    toggle_state: HashMap<KeyCode, bool>,
    /// Tokio runtime handle for spawning tasks
    runtime: Option<tokio::runtime::Handle>,
}

impl MacroEngine {
    pub fn new(writer: Arc<Mutex<DeviceWriter>>) -> Self {
        Self {
            writer,
            active: HashMap::new(),
            toggle_state: HashMap::new(),
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    /// Start a macro for the given trigger key
    pub fn start_macro(&mut self, trigger: KeyCode, macro_def: &MacroDef) -> Result<()> {
        // Ensure we have a runtime handle
        let handle = match &self.runtime {
            Some(h) => h.clone(),
            None => {
                // Try to get one now
                match tokio::runtime::Handle::try_current() {
                    Ok(h) => {
                        self.runtime = Some(h.clone());
                        h
                    }
                    Err(_) => {
                        log::error!("No tokio runtime available for macro execution");
                        return Ok(());
                    }
                }
            }
        };

        match macro_def.macro_type {
            MacroType::RepeatOnHold => {
                // If already running, ignore (key repeat events)
                if self.active.contains_key(&trigger) {
                    return Ok(());
                }

                let (cancel_tx, cancel_rx) = watch::channel(false);
                self.active.insert(trigger, cancel_tx);

                let writer = self.writer.clone();
                let actions = macro_def.actions.clone();
                let interval = std::time::Duration::from_millis(macro_def.interval_ms);
                let jitter_ms = macro_def.jitter_ms;
                let initial_delay = if macro_def.initial_delay_ms > 0 {
                    Some(std::time::Duration::from_millis(macro_def.initial_delay_ms))
                } else {
                    None
                };

                handle.spawn(async move {
                    run_repeat_macro(writer, actions, interval, jitter_ms, initial_delay, cancel_rx)
                        .await;
                });
            }

            MacroType::Sequence => {
                let writer = self.writer.clone();
                let actions = macro_def.actions.clone();

                handle.spawn(async move {
                    run_sequence_macro(writer, actions).await;
                });
            }

            MacroType::Toggle => {
                let is_active = self.toggle_state.get(&trigger).copied().unwrap_or(false);

                if is_active {
                    // Stop the toggle
                    self.toggle_state.insert(trigger, false);
                    if let Some(tx) = self.active.remove(&trigger) {
                        let _ = tx.send(true); // Signal cancellation
                    }
                } else {
                    // Start the toggle
                    self.toggle_state.insert(trigger, true);

                    let (cancel_tx, cancel_rx) = watch::channel(false);
                    self.active.insert(trigger, cancel_tx);

                    let writer = self.writer.clone();
                    let actions = macro_def.actions.clone();
                    let interval = std::time::Duration::from_millis(macro_def.interval_ms);
                    let jitter_ms = macro_def.jitter_ms;

                    handle.spawn(async move {
                        run_repeat_macro(writer, actions, interval, jitter_ms, None, cancel_rx)
                            .await;
                    });
                }
            }
        }

        Ok(())
    }

    /// Stop a macro for the given trigger key
    pub fn stop_macro(&mut self, trigger: KeyCode) {
        // For toggle macros, don't stop on release - they stop on next press
        if self.toggle_state.get(&trigger).copied().unwrap_or(false) {
            return;
        }

        if let Some(tx) = self.active.remove(&trigger) {
            let _ = tx.send(true); // Signal cancellation
        }
    }

    /// Stop all running macros
    pub fn stop_all(&mut self) {
        for (_, tx) in self.active.drain() {
            let _ = tx.send(true);
        }
        self.toggle_state.clear();
    }
}

/// Run a repeating macro (used for both RepeatOnHold and Toggle)
async fn run_repeat_macro(
    writer: Arc<Mutex<DeviceWriter>>,
    actions: Vec<MacroAction>,
    interval: std::time::Duration,
    jitter_ms: u64,
    initial_delay: Option<std::time::Duration>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    if let Some(delay) = initial_delay {
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = cancel_rx.changed() => { return; }
        }
    }

    let mut rng = StdRng::from_entropy();

    loop {
        // Execute all actions in the sequence
        for action in &actions {
            if *cancel_rx.borrow() {
                return;
            }
            execute_action(&writer, action);
        }

        // Compute sleep duration with random jitter
        let sleep_duration = if jitter_ms > 0 {
            let base_ms = interval.as_millis() as i64;
            let jitter = jitter_ms as i64;
            let offset = rng.gen_range(-jitter..=jitter);
            let actual_ms = (base_ms + offset).max(1) as u64;
            log::debug!(
                "repeat sleep: {}ms (base={}ms, jitter=\u{00b1}{}ms, offset={:+}ms)",
                actual_ms,
                base_ms,
                jitter_ms,
                offset
            );
            std::time::Duration::from_millis(actual_ms)
        } else {
            interval
        };

        // Wait for the (jittered) interval or cancellation
        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {}
            _ = cancel_rx.changed() => { return; }
        }
    }
}

/// Run a sequence macro (fires once)
async fn run_sequence_macro(writer: Arc<Mutex<DeviceWriter>>, actions: Vec<MacroAction>) {
    for action in &actions {
        execute_action_async(&writer, action).await;
    }
}

/// Execute a single macro action (blocking)
fn execute_action(writer: &Arc<Mutex<DeviceWriter>>, action: &MacroAction) {
    let mut writer = match writer.lock() {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to lock writer: {}", e);
            return;
        }
    };

    match action {
        MacroAction::Click(key_name) => {
            if let Some(key) = parse_key_name(key_name) {
                if let Err(e) = writer.click(key) {
                    log::error!("Failed to click {}: {}", key_name, e);
                }
            }
        }
        MacroAction::Press(key_name) => {
            if let Some(key) = parse_key_name(key_name) {
                if let Err(e) = writer.press(key) {
                    log::error!("Failed to press {}: {}", key_name, e);
                }
            }
        }
        MacroAction::Release(key_name) => {
            if let Some(key) = parse_key_name(key_name) {
                if let Err(e) = writer.release(key) {
                    log::error!("Failed to release {}: {}", key_name, e);
                }
            }
        }
        MacroAction::Delay(_) => {
            // Delays are handled in the async version
        }
    }
}

/// Execute a single macro action (async, supports delays)
async fn execute_action_async(writer: &Arc<Mutex<DeviceWriter>>, action: &MacroAction) {
    match action {
        MacroAction::Delay(ms) => {
            tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
        }
        other => {
            execute_action(writer, other);
        }
    }
}
