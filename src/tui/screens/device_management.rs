use crate::device::DeviceInfo;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Cell, Paragraph, Row, Table, TableState},
};

pub fn render_device_management_screen(
    frame: &mut Frame,
    area: Rect,
    devices: &[DeviceInfo],
    table_state: &mut TableState,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .flex(ratatui::layout::Flex::SpaceBetween)
        .split(area);

    render_device_list(devices, frame, layout[0], table_state);
    render_device_details(devices, frame, layout[1], table_state);
}

fn render_device_list(
    devices: &[DeviceInfo],
    frame: &mut Frame,
    area: Rect,
    table_state: &mut TableState,
) {
    let header_cells = [" Device ID", "Name", "evdev Path"]
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
                Cell::from(device_data.get_device_name()),
                Cell::from(device_data.evdev_path.clone()),
            ])
        })
        .collect::<Vec<Row>>();

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(40),
        Constraint::Percentage(30),
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
    let Some(device) = table_state.selected().and_then(|i| devices.get(i)) else {
        frame.render_widget(
            Paragraph::new("No device selected.")
                .block(Block::bordered().title(" Device Details "))
                .fg(Color::DarkGray),
            area,
        );
        return;
    };

    let evdev = &device.evdev_device;
    let input_id = evdev.input_id();
    let (major, minor, patch) = evdev.driver_version();
    let key_count = evdev
        .supported_keys()
        .map(|keys| keys.iter().count())
        .unwrap_or(0);

    let rows = [
        ("Name", device.get_device_name()),
        ("Vendor ID", format!("{:04x}", device.vendor_id)),
        ("Product ID", format!("{:04x}", device.product_id)),
        ("MAC Address", device.get_mac_address()),
        ("evdev Path", device.evdev_path.clone()),
        (
            "Physical Path",
            evdev.physical_path().unwrap_or("-").to_string(),
        ),
        (
            "Unique Name",
            evdev.unique_name().unwrap_or("-").to_string(),
        ),
        ("Bus Type", input_id.bus_type().to_string()),
        ("Driver Version", format!("{major}.{minor}.{patch}")),
        ("Supported Keys", key_count.to_string()),
    ];

    let detail_rows = rows.into_iter().map(|(label, value)| {
        Row::new(vec![
            Cell::from(format!(" {label}")).fg(Color::Gray).bold(),
            Cell::from(value).fg(Color::White),
        ])
    });

    let table = Table::new(
        detail_rows,
        [Constraint::Percentage(35), Constraint::Percentage(65)],
    )
    .block(Block::bordered().title(format!(" Device Details: [{}] ", device.get_device_name())))
    .column_spacing(1);

    frame.render_widget(table, area);
}
