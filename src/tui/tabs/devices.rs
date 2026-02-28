use crate::tui::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let devices = &app.devices;

    if devices.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from("No input devices found."),
            Line::from(""),
            Line::from("Make sure you're running as root (sudo)."),
            Line::from("Press 'r' to refresh the device list."),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Devices "));
        f.render_widget(msg, area);
        return;
    }

    let header_cells = ["Path", "Name", "VID:PID", "Type", "Capabilities"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = devices
        .iter()
        .enumerate()
        .map(|(_i, device)| {
            let selected = app
                .selected_device
                .as_ref()
                .is_some_and(|d| d.path == device.path);

            let type_str = if device.is_mouse { "Mouse" } else { "Other" };
            let vid_pid = format!("{:04x}:{:04x}", device.vendor_id, device.product_id);

            let style = if selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if device.is_mouse {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let prefix = if selected { "* " } else { "  " };

            Row::new(vec![
                Cell::from(format!("{}{}", prefix, device.path.display())),
                Cell::from(device.name.clone()),
                Cell::from(vid_pid),
                Cell::from(type_str),
                Cell::from(device.capabilities.clone()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(22),
        Constraint::Min(30),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Devices (Enter=select, r=refresh, Space=start/stop engine) "),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = TableState::default();
    state.select(Some(app.device_list_index));

    f.render_stateful_widget(table, area, &mut state);
}
