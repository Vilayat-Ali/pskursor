use crate::device::{DeviceInfo, get_devices};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Cell, Row, Table, TableState},
};

pub fn render_device_management_screen(frame: &mut Frame, area: Rect) {
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    let devices = get_devices().unwrap();

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(38)])
        .flex(ratatui::layout::Flex::SpaceBetween)
        .split(area);

    render_device_list(&devices, frame, layout[0], &mut table_state);
    render_device_details(&devices, frame, layout[1], &table_state);
}

fn render_device_list(
    devices: &[DeviceInfo],
    frame: &mut Frame,
    area: Rect,
    table_state: &mut TableState,
) {
    let header_cells = [
        " Device ID",
        "Vendor ID",
        "Name",
        "Mac Address",
        "Device Path",
    ]
    .into_iter()
    .map(|h| Cell::from(h).fg(Color::White).bold());

    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Black))
        .height(1);

    let data_rows = devices
        .iter()
        .map(|device_data| {
            Row::new(vec![
                Cell::from(format!(" {}", device_data.product_id)),
                Cell::from(device_data.vendor_id.to_string()),
                Cell::from(device_data.get_device_name()),
                Cell::from(device_data.get_mac_address()),
                Cell::from(device_data.path.to_string_lossy()),
            ])
        })
        .collect::<Vec<Row>>();

    let widths = [
        Constraint::Percentage(12),
        Constraint::Percentage(12),
        Constraint::Percentage(34),
        Constraint::Percentage(16),
        Constraint::Percentage(15),
    ];

    let table = Table::new(data_rows, widths)
        .block(Block::bordered().title(format!(" Connected Devices ({}) ", devices.len())))
        .header(header)
        .style(Style::default().fg(Color::White))
        .row_highlight_style(Style::default().bg(Color::White).fg(Color::Black))
        .column_spacing(2);

    frame.render_stateful_widget(table, area, table_state);
}

fn render_device_details(
    devices: &[DeviceInfo],
    frame: &mut Frame,
    area: Rect,
    table_state: &TableState,
) {
    let device_data = &devices[table_state.selected().unwrap_or_default()];
    let block = Block::bordered().title(format!(
        " Device Details: [{}]",
        device_data.get_device_name()
    ));

    frame.render_widget(block, area);
}
