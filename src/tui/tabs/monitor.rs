use crate::tui::app::{App, EngineMessage};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.monitor_paused {
        " Monitor [PAUSED] (p=toggle pause, c=clear) "
    } else {
        " Monitor [LIVE] (p=toggle pause, c=clear) "
    };

    if app.monitor_events.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from("No events captured yet."),
            Line::from(""),
            Line::from("Start the engine (Space on Devices tab) to see live events."),
            Line::from("This shows all raw input events from the grabbed device."),
            Line::from(""),
            Line::from("Useful for finding button codes for your mouse."),
        ])
        .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(msg, area);
        return;
    }

    // Show the most recent events that fit in the area
    let visible_height = area.height.saturating_sub(2) as usize; // account for borders
    let start = if app.monitor_events.len() > visible_height {
        app.monitor_events.len() - visible_height
    } else {
        0
    };

    let lines: Vec<Line> = app.monitor_events[start..]
        .iter()
        .map(|msg| match msg {
            EngineMessage::RawEvent {
                event_type,
                code,
                value,
                timestamp,
            } => {
                let color = if event_type.contains("KEY") {
                    if *value == 1 {
                        Color::Green
                    } else if *value == 0 {
                        Color::Red
                    } else {
                        Color::Yellow
                    }
                } else if event_type.contains("REL") {
                    Color::Cyan
                } else {
                    Color::DarkGray
                };

                let value_str = match *value {
                    0 => "UP  ".to_string(),
                    1 => "DOWN".to_string(),
                    2 => "REPT".to_string(),
                    v => format!("{:4}", v),
                };

                Line::from(vec![
                    Span::styled(
                        format!("{} ", timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:12} ", event_type),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(format!("{:20} ", code), Style::default().fg(color)),
                    Span::styled(
                        value_str,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                ])
            }
            EngineMessage::StatusUpdate(s) => Line::from(Span::styled(
                format!("  [STATUS] {}", s),
                Style::default().fg(Color::Blue),
            )),
            EngineMessage::Error(e) => Line::from(Span::styled(
                format!("  [ERROR] {}", e),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if app.monitor_paused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            }),
    );

    f.render_widget(paragraph, area);
}
