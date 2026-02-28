use crate::config::MacroType;
use crate::tui::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let macros = app.current_macros();

    if macros.is_empty() && app.editing_macro.is_none() {
        let msg = Paragraph::new(vec![
            Line::from("No macros configured for the active profile."),
            Line::from(""),
            Line::from("Press 'a' to add a new macro."),
            Line::from(""),
            Line::from("Macros can repeat clicks while a button is held,"),
            Line::from("play a sequence of key presses, or toggle repeating."),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Macros (a=add, e=edit, d=delete, s=save config) "),
        );
        f.render_widget(msg, area);
    } else if app.editing_macro.is_none() {
        let header_cells = ["Name", "Type", "Actions", "Interval", "Jitter"]
            .iter()
            .map(|h| {
                Cell::from(*h).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            });
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = macros
            .iter()
            .map(|m| {
                let type_str = match m.macro_type {
                    MacroType::RepeatOnHold => "Repeat on Hold",
                    MacroType::Sequence => "Sequence",
                    MacroType::Toggle => "Toggle",
                };

                let actions_str = m
                    .actions
                    .iter()
                    .map(|a| format!("{:?}", a))
                    .collect::<Vec<_>>()
                    .join(", ");

                let interval = format!("{}ms", m.interval_ms);
                let jitter = if m.jitter_ms > 0 {
                    format!("\u{00b1}{}ms", m.jitter_ms)
                } else {
                    "off".to_string()
                };

                Row::new(vec![
                    Cell::from(m.name.clone()),
                    Cell::from(type_str),
                    Cell::from(actions_str),
                    Cell::from(interval),
                    Cell::from(jitter),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(20),
            Constraint::Length(16),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(10),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Macros (a=add, e=edit, d=delete, s=save config) "),
            )
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut state = TableState::default();
        state.select(Some(app.macro_list_index));

        f.render_stateful_widget(table, area, &mut state);
    }

    // Render edit dialog if active
    if let Some(ref editing) = app.editing_macro {
        render_edit_dialog(f, editing, area);
    }
}

fn render_edit_dialog(f: &mut Frame, editing: &crate::tui::app::EditingMacro, area: Rect) {
    let dialog_width = 65.min(area.width.saturating_sub(4));
    let dialog_height = 19.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    let title = if editing.index.is_some() {
        " Edit Macro "
    } else {
        " New Macro "
    };

    let type_str = match editing.macro_type {
        MacroType::RepeatOnHold => "Repeat on Hold",
        MacroType::Sequence => "Sequence",
        MacroType::Toggle => "Toggle",
    };

    let actions_str = editing
        .actions
        .iter()
        .map(|a| format!("{:?}", a))
        .collect::<Vec<_>>()
        .join(", ");

    let field_indicator = |idx: usize| -> &str {
        if editing.field_index == idx {
            " <<"
        } else {
            ""
        }
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Name:     ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!(
                    "[{}]",
                    if editing.name.is_empty() {
                        "<enter name>"
                    } else {
                        &editing.name
                    }
                ),
                if editing.field_index == 0 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw(field_indicator(0)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Type:     ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{}]", type_str),
                if editing.field_index == 1 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw(field_indicator(1)),
            Span::styled("  (Tab to cycle)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Actions:  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!(
                    "[{}]",
                    if actions_str.is_empty() {
                        "<add actions>"
                    } else {
                        &actions_str
                    }
                ),
                if editing.field_index == 2 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw(field_indicator(2)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Interval: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{}ms]", editing.interval_ms),
                if editing.field_index == 3 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw(field_indicator(3)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Jitter:   ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!(
                    "[\u{00b1}{}ms]",
                    if editing.jitter_ms.is_empty() {
                        "0"
                    } else {
                        &editing.jitter_ms
                    }
                ),
                if editing.field_index == 4 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw(field_indicator(4)),
            if editing.field_index == 4 {
                Span::styled(
                    "  (random timing variance)",
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Up/Down=navigate  Tab=cycle type  Enter=save  Esc=cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(paragraph, dialog_area);
}
