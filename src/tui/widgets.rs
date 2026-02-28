use crate::tui::app::{App, Tab};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

/// Render the top tab bar
pub fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .map(|t| {
            let style = if *t == app.current_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(t.title(), style))
        })
        .collect();

    let selected = Tab::all()
        .iter()
        .position(|t| *t == app.current_tab)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Mouse Mapper "),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" | "));

    f.render_widget(tabs, area);
}

/// Render the bottom status bar
pub fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let engine_status = if app.engine_running {
        Span::styled(
            " ENGINE: RUNNING ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " ENGINE: STOPPED ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
    };

    let device_info = if let Some(ref device) = app.selected_device {
        Span::styled(
            format!(" Device: {} ", device.name),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::styled(" No device selected ", Style::default().fg(Color::Yellow))
    };

    let profile_name = app
        .config
        .active_profile()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "None".to_string());

    let status = Line::from(vec![
        engine_status,
        Span::raw(" "),
        device_info,
        Span::raw(" | "),
        Span::styled(
            format!("Profile: {}", profile_name),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" | "),
        Span::styled(&app.status_message, Style::default().fg(Color::White)),
    ]);

    let paragraph = Paragraph::new(status).block(Block::default().borders(Borders::TOP));

    f.render_widget(paragraph, area);
}

/// Render a help overlay
pub fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            " Mouse Mapper - Keyboard Shortcuts ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(" Global:", Style::default().fg(Color::Yellow))),
        Line::from("   Left/Right or H/L  Switch tabs"),
        Line::from("   q                   Quit"),
        Line::from("   s                   Save config to disk"),
        Line::from("   ?                   Toggle this help"),
        Line::from(""),
        Line::from(Span::styled(
            " Devices Tab:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("   Up/Down or J/K      Navigate device list"),
        Line::from("   Enter               Select device"),
        Line::from("   Space               Start/stop engine"),
        Line::from("   r                   Refresh device list"),
        Line::from(""),
        Line::from(Span::styled(
            " Bindings/Macros Tab:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("   Up/Down or J/K      Navigate list"),
        Line::from("   a                   Add new entry"),
        Line::from("   e                   Edit selected entry"),
        Line::from("   d                   Delete selected entry"),
        Line::from(""),
        Line::from(Span::styled(
            " Edit Dialog:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("   Up/Down             Navigate fields"),
        Line::from("   Tab                 Cycle through options"),
        Line::from("   Enter               Save"),
        Line::from("   Esc                 Cancel"),
        Line::from(""),
        Line::from(Span::styled(
            " Monitor Tab:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("   p                   Pause/resume"),
        Line::from("   c                   Clear events"),
    ];

    // Center the help dialog
    let dialog_width = 55.min(area.width.saturating_sub(4));
    let dialog_height = (help_text.len() as u16 + 2).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(ratatui::widgets::Clear, dialog_area);

    let paragraph = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(paragraph, dialog_area);
}
