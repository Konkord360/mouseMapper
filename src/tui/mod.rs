pub mod app;
pub mod tabs;
pub mod widgets;

use crate::config::MacroType;
use crate::tui::app::{App, BindingOutputType, EngineCommand, InputMode, Tab};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    Terminal,
};
use std::io;
use std::time::Duration;

/// Run the TUI event loop
pub fn run(mut app: App) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initial device scan
    app.refresh_devices();

    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Tell engine to shut down
    if let Some(ref tx) = app.engine_cmd_tx {
        let _ = tx.send(EngineCommand::Shutdown);
    }

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut show_help = false;

    loop {
        // Poll engine messages
        app.poll_engine_messages();

        // Draw
        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([
                    Constraint::Length(3), // tab bar
                    Constraint::Min(1),    // main content
                    Constraint::Length(3), // status bar
                ])
                .split(f.area());

            widgets::render_tabs(f, app, chunks[0]);

            match app.current_tab {
                Tab::Devices => tabs::devices::render(f, app, chunks[1]),
                Tab::Bindings => tabs::bindings::render(f, app, chunks[1]),
                Tab::Macros => tabs::macros::render(f, app, chunks[1]),
                Tab::Monitor => tabs::monitor::render(f, app, chunks[1]),
            }

            widgets::render_status_bar(f, app, chunks[2]);

            if show_help {
                widgets::render_help(f, f.area());
            }
        })?;

        if app.should_quit {
            return Ok(());
        }

        // Handle input with a small timeout so we can poll engine messages
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Global: Ctrl+C always quits
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                    continue;
                }

                // Help toggle
                if key.code == KeyCode::Char('?') && app.input_mode == InputMode::Normal {
                    show_help = !show_help;
                    continue;
                }

                if show_help {
                    // Any key closes help
                    show_help = false;
                    continue;
                }

                // Handle based on input mode
                match &app.input_mode {
                    InputMode::Normal => {
                        handle_normal_input(app, key.code)?;
                    }
                    InputMode::Editing(_) => {
                        handle_editing_input(app, key.code, key.modifiers);
                    }
                    InputMode::Capturing { .. } => {
                        // In capture mode, any key is recorded
                        handle_capture_input(app, key.code);
                    }
                    InputMode::Confirming(_) => {
                        handle_confirm_input(app, key.code);
                    }
                }
            }
        }
    }
}

fn handle_normal_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        // Quit
        KeyCode::Char('q') => {
            app.should_quit = true;
        }

        // Tab navigation
        KeyCode::Right | KeyCode::Char('l') => {
            app.current_tab = app.current_tab.next();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.current_tab = app.current_tab.prev();
        }

        // Save config
        KeyCode::Char('s') => {
            app.save_config();
        }

        // Tab-specific keys
        _ => match app.current_tab {
            Tab::Devices => handle_devices_input(app, key),
            Tab::Bindings => handle_bindings_input(app, key),
            Tab::Macros => handle_macros_input(app, key),
            Tab::Monitor => handle_monitor_input(app, key),
        },
    }

    Ok(())
}

fn handle_devices_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.device_list_index > 0 {
                app.device_list_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.device_list_index + 1 < app.devices.len() {
                app.device_list_index += 1;
            }
        }
        KeyCode::Enter => {
            app.select_current_device();
        }
        KeyCode::Char(' ') => {
            app.toggle_engine();
        }
        KeyCode::Char('r') => {
            app.refresh_devices();
        }
        _ => {}
    }
}

fn handle_bindings_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.binding_list_index > 0 {
                app.binding_list_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.current_bindings().len();
            if app.binding_list_index + 1 < len {
                app.binding_list_index += 1;
            }
        }
        KeyCode::Char('a') => {
            app.start_new_binding();
        }
        KeyCode::Char('e') => {
            app.start_edit_binding();
        }
        KeyCode::Char('d') => {
            app.input_mode = InputMode::Confirming("Delete this binding?".to_string());
        }
        _ => {}
    }
}

fn handle_macros_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.macro_list_index > 0 {
                app.macro_list_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.current_macros().len();
            if app.macro_list_index + 1 < len {
                app.macro_list_index += 1;
            }
        }
        KeyCode::Char('a') => {
            app.start_new_macro();
        }
        KeyCode::Char('e') => {
            app.start_edit_macro();
        }
        KeyCode::Char('d') => {
            app.input_mode = InputMode::Confirming("Delete this macro?".to_string());
        }
        _ => {}
    }
}

fn handle_monitor_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('p') => {
            app.monitor_paused = !app.monitor_paused;
            if app.monitor_paused {
                app.set_status("Monitor paused");
            } else {
                app.set_status("Monitor resumed");
            }
        }
        KeyCode::Char('c') => {
            app.monitor_events.clear();
            app.set_status("Monitor cleared");
        }
        _ => {}
    }
}

fn handle_editing_input(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    // Ctrl+S always saves (binding or macro)
    if modifiers.contains(KeyModifiers::CONTROL) && key == KeyCode::Char('s') {
        if app.editing_binding.is_some() {
            app.save_editing_binding();
        } else if app.editing_macro.is_some() {
            app.save_editing_macro();
        }
        return;
    }

    // Dispatch to binding-specific or macro-specific handler
    if app.editing_binding.is_some() {
        handle_editing_binding_input(app, key);
    } else if app.editing_macro.is_some() {
        handle_editing_macro_input(app, key);
    }
}

fn handle_editing_binding_input(app: &mut App, key: KeyCode) {
    // Determine current field_index and output_type before borrow
    let (field_index, is_macro_output, is_key_output) = {
        let editing = app.editing_binding.as_ref().unwrap();
        (
            editing.field_index,
            editing.output_type == BindingOutputType::Macro,
            editing.output_type == BindingOutputType::Key,
        )
    };

    match key {
        KeyCode::Esc => {
            app.editing_binding = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            match field_index {
                // Field 0: input button — start capture
                0 => {
                    app.start_capture(app::CaptureField::BindingInput);
                }
                // Field 1: output type — no action on Enter (use Tab to toggle)
                1 => {}
                // Field 2: output value
                2 => {
                    if is_key_output {
                        // Start capture for key output
                        app.start_capture(app::CaptureField::BindingOutput);
                    } else if is_macro_output {
                        // Select the currently highlighted macro
                        let macro_names = app.macro_names();
                        if let Some(editing) = app.editing_binding.as_mut() {
                            if let Some(name) = macro_names.get(editing.macro_select_index) {
                                editing.output_value = name.clone();
                                app.set_status(format!("Selected macro: {}", name));
                            }
                        }
                        // Save the binding after selecting a macro
                        app.save_editing_binding();
                    }
                }
                _ => {}
            }
        }
        KeyCode::Up => {
            // On field 2 with Macro output: navigate macro list
            if field_index == 2 && is_macro_output {
                if let Some(ref mut editing) = app.editing_binding {
                    if editing.macro_select_index > 0 {
                        editing.macro_select_index -= 1;
                    }
                }
            } else if let Some(ref mut editing) = app.editing_binding {
                if editing.field_index > 0 {
                    editing.field_index -= 1;
                }
            }
        }
        KeyCode::Down => {
            // On field 2 with Macro output: navigate macro list
            if field_index == 2 && is_macro_output {
                let macro_count = app.macro_names().len();
                if let Some(ref mut editing) = app.editing_binding {
                    if editing.macro_select_index + 1 < macro_count {
                        editing.macro_select_index += 1;
                    }
                }
            } else if let Some(ref mut editing) = app.editing_binding {
                if editing.field_index < 2 {
                    editing.field_index += 1;
                }
            }
        }
        KeyCode::Tab => {
            if let Some(ref mut editing) = app.editing_binding {
                if editing.field_index == 1 {
                    editing.output_type = match editing.output_type {
                        BindingOutputType::Key => BindingOutputType::Macro,
                        BindingOutputType::Macro => BindingOutputType::Key,
                    };
                    // Reset output_value when switching types
                    editing.output_value.clear();
                    editing.macro_select_index = 0;
                }
            }
        }
        KeyCode::Backspace => {
            // Only allow manual text editing for fields that aren't capture-based
            // Field 0 and field 2 (Key) are capture-only, so backspace clears them
            if let Some(ref mut editing) = app.editing_binding {
                match field_index {
                    0 => {
                        editing.input.clear();
                    }
                    2 if is_key_output => {
                        editing.output_value.clear();
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char(_) => {
            // No manual typing for binding fields — use capture for input/key output,
            // use list selection for macro output. This prevents mistyped key names.
        }
        _ => {}
    }
}

fn handle_editing_macro_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.editing_macro = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.save_editing_macro();
        }
        KeyCode::Up => {
            if let Some(ref mut editing) = app.editing_macro {
                if editing.field_index > 0 {
                    editing.field_index -= 1;
                }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut editing) = app.editing_macro {
                if editing.field_index < 3 {
                    editing.field_index += 1;
                }
            }
        }
        KeyCode::Tab => {
            if let Some(ref mut editing) = app.editing_macro {
                if editing.field_index == 1 {
                    editing.macro_type = match editing.macro_type {
                        MacroType::RepeatOnHold => MacroType::Sequence,
                        MacroType::Sequence => MacroType::Toggle,
                        MacroType::Toggle => MacroType::RepeatOnHold,
                    };
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut editing) = app.editing_macro {
                match editing.field_index {
                    0 => {
                        editing.name.pop();
                    }
                    3 => {
                        editing.interval_ms.pop();
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut editing) = app.editing_macro {
                match editing.field_index {
                    0 => editing.name.push(c),
                    2 => {
                        if editing.actions.is_empty() {
                            editing
                                .actions
                                .push(crate::config::MacroAction::Click(String::new()));
                        }
                        if let Some(crate::config::MacroAction::Click(s)) =
                            editing.actions.first_mut()
                        {
                            s.push(c);
                        }
                    }
                    3 => {
                        if c.is_ascii_digit() {
                            editing.interval_ms.push(c);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn handle_capture_input(app: &mut App, key: KeyCode) {
    // In capture mode, the actual button capture comes from the evdev background task
    // via poll_capture(). The only keyboard input we handle here is Esc to cancel.
    if key == KeyCode::Esc {
        app.capturing = false;
        app.input_mode = InputMode::Editing(String::new());
        app.set_status("Capture cancelled");
    }
    // All other keyboard keys are ignored — we're waiting for a mouse button via evdev
}

fn handle_confirm_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('y') | KeyCode::Enter => {
            // Confirmed
            match app.current_tab {
                Tab::Bindings => app.delete_current_binding(),
                Tab::Macros => app.delete_current_macro(),
                _ => {}
            }
            app.input_mode = InputMode::Normal;
        }
        _ => {
            // Cancelled
            app.input_mode = InputMode::Normal;
            app.set_status("Cancelled");
        }
    }
}
