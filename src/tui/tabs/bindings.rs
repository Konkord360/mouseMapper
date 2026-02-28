use crate::config::BindingOutput;
use crate::tui::app::{App, BindingOutputType, InputMode};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let bindings = app.current_bindings();

    if bindings.is_empty() && app.editing_binding.is_none() {
        let msg = Paragraph::new(vec![
            Line::from("No bindings configured for the active profile."),
            Line::from(""),
            Line::from("Press 'a' to add a new binding."),
            Line::from(""),
            Line::from("Bindings remap mouse buttons to other keys/buttons,"),
            Line::from("or trigger macros when pressed."),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Bindings (a=add, e=edit, d=delete, s=save config) "),
        );
        f.render_widget(msg, area);
    } else if app.editing_binding.is_none() {
        // Show binding list
        let header_cells = ["Input Button", "Action", "Output"].iter().map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = bindings
            .iter()
            .map(|binding| {
                let (action, output) = match &binding.output {
                    BindingOutput::Key { key } => ("Key Remap", key.clone()),
                    BindingOutput::Macro { macro_name } => ("Macro", macro_name.clone()),
                };

                Row::new(vec![
                    Cell::from(binding.input.clone()),
                    Cell::from(action),
                    Cell::from(output),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(20),
            Constraint::Length(15),
            Constraint::Min(20),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Bindings (a=add, e=edit, d=delete, s=save config) "),
            )
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut state = TableState::default();
        state.select(Some(app.binding_list_index));

        f.render_stateful_widget(table, area, &mut state);
    }

    // Render edit dialog if active
    if app.editing_binding.is_some() {
        render_edit_dialog(f, app, area);
    }
}

fn render_edit_dialog(f: &mut Frame, app: &App, area: Rect) {
    let editing = app.editing_binding.as_ref().unwrap();
    let is_capturing = matches!(app.input_mode, InputMode::Capturing { .. });
    let macro_names = app.macro_names();
    let is_macro_output = editing.output_type == BindingOutputType::Macro;

    // Increase dialog height when showing macro list
    let base_height: u16 = 14;
    let macro_list_extra: u16 = if is_macro_output && editing.field_index == 2 {
        (macro_names.len() as u16).min(6).max(1) + 1 // +1 for label
    } else {
        0
    };
    let dialog_height = (base_height + macro_list_extra).min(area.height.saturating_sub(4));

    // Center the dialog
    let dialog_width = 60.min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    let title = if editing.index.is_some() {
        " Edit Binding "
    } else {
        " New Binding "
    };

    let output_type_str = match editing.output_type {
        BindingOutputType::Key => "Key Remap",
        BindingOutputType::Macro => "Macro",
    };

    let field_indicator = |idx: usize| -> &str {
        if editing.field_index == idx {
            " <<"
        } else {
            ""
        }
    };

    let focused_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let unfocused_style = Style::default().fg(Color::Gray);
    let hint_style = Style::default().fg(Color::DarkGray);

    // Field 0: Input button
    let input_display = if is_capturing && editing.field_index == 0 {
        "[Waiting for button press... (Esc to cancel)]".to_string()
    } else if editing.input.is_empty() {
        "[<Enter to capture>]".to_string()
    } else {
        format!("[{}]", editing.input)
    };

    let input_style = if is_capturing && editing.field_index == 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if editing.field_index == 0 {
        focused_style
    } else {
        unfocused_style
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Input button: ", Style::default().fg(Color::Yellow)),
            Span::styled(input_display, input_style),
            Span::raw(field_indicator(0)),
            if editing.field_index == 0 && !is_capturing {
                Span::styled("  (Enter to capture)", hint_style)
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Output type:  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{}]", output_type_str),
                if editing.field_index == 1 {
                    focused_style
                } else {
                    unfocused_style
                },
            ),
            Span::raw(field_indicator(1)),
            Span::styled("  (Tab to toggle)", hint_style),
        ]),
        Line::from(""),
    ];

    // Field 2: Output value — different rendering based on output type
    if is_macro_output {
        // Macro output: show a selectable list
        let output_label = if editing.field_index == 2 {
            "  Output macro: "
        } else {
            "  Output macro: "
        };
        let current_value = if editing.output_value.is_empty() {
            "<none selected>"
        } else {
            &editing.output_value
        };

        lines.push(Line::from(vec![
            Span::styled(output_label, Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{}]", current_value),
                if editing.field_index == 2 {
                    focused_style
                } else {
                    unfocused_style
                },
            ),
            Span::raw(field_indicator(2)),
        ]));

        // Show macro list when field 2 is focused
        if editing.field_index == 2 {
            if macro_names.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    No macros -- create one in the Macros tab first",
                    Style::default().fg(Color::Red),
                )));
            } else {
                for (i, name) in macro_names.iter().enumerate() {
                    let is_selected = i == editing.macro_select_index;
                    let prefix = if is_selected { "  > " } else { "    " };
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", prefix, name),
                        style,
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "    (Up/Down to select, Enter to confirm)",
                    hint_style,
                )));
            }
        }
    } else {
        // Key output: capture-based
        let output_display = if is_capturing && editing.field_index == 2 {
            "[Waiting for button press... (Esc to cancel)]".to_string()
        } else if editing.output_value.is_empty() {
            "[<Enter to capture>]".to_string()
        } else {
            format!("[{}]", editing.output_value)
        };

        let output_style = if is_capturing && editing.field_index == 2 {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if editing.field_index == 2 {
            focused_style
        } else {
            unfocused_style
        };

        lines.push(Line::from(vec![
            Span::styled("  Output key:   ", Style::default().fg(Color::Yellow)),
            Span::styled(output_display, output_style),
            Span::raw(field_indicator(2)),
            if editing.field_index == 2 && !is_capturing {
                Span::styled("  (Enter to capture)", hint_style)
            } else {
                Span::raw("")
            },
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Up/Down=fields  Ctrl+S=save  Esc=cancel",
        hint_style,
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(paragraph, dialog_area);
}
