use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

pub fn render_control_help(frame: &mut Frame, area: Rect) {
    let sections = [
        (
            "1. Device Management",
            vec![
                (" d ", "Navigate to Device Management page"),
                (" ▲ ", "Move up in device list table"),
                (" ▼ ", "Move down in device list table"),
            ],
        ),
        (
            "2. Device State",
            vec![(" s ", "Navigate to device stats page")],
        ),
    ];

    let num_columns = 2;

    let chunks = if num_columns > 1 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(100 / num_columns as u16);
                num_columns
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(7), Constraint::Length(5)])
            .split(area)
    };

    for (i, (title, shortcuts)) in sections.iter().enumerate() {
        if i >= chunks.len() {
            break;
        }

        let mut rows = Vec::new();
        for (key, desc) in shortcuts {
            rows.push(Row::new(vec![
                Cell::from(format!(" {} ", key)).style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(format!(" {}", desc)).style(Style::default().fg(Color::White)),
            ]));
        }

        let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(20)]).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", title))
                .border_style(Style::default().fg(Color::LightGreen)),
        );

        frame.render_widget(table, chunks[i]);
    }
}
