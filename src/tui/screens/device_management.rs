use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Cell, Paragraph, Row, Table, TableState},
};

use crate::device::{DeviceInfo, missing_gamepad_capabilities};

/// evdev metadata for one device, read once per selection change.
///
/// Opening `/dev/input/event*` can fail (permission denied, device unplugged
/// between enumeration and inspection), so the failure is captured here and
/// rendered instead of panicking mid-frame.
pub struct DeviceDetails {
    rows: Vec<(&'static str, String)>,
    error: Option<String>,
    title: String,
}

impl DeviceDetails {
    pub fn probe(device: &DeviceInfo) -> Self {
        let title = device.get_device_name();

        let mut rows = vec![
            ("Name", device.get_device_name()),
            ("Vendor ID", format!("{:04x}", device.vendor_id)),
            ("Product ID", format!("{:04x}", device.product_id)),
            ("MAC Address", device.get_mac_address()),
            ("evdev Path", device.evdev_path.clone()),
        ];

        let error = match device.open() {
            Ok(evdev) => {
                rows.push((
                    "Cursor Capable",
                    match missing_gamepad_capabilities(&evdev) {
                        None => "yes — gamepad with X/Y axes".to_string(),
                        Some(missing) => format!("no — {missing}"),
                    },
                ));

                let input_id = evdev.input_id();
                let (major, minor, patch) = evdev.driver_version();
                let key_count = evdev
                    .supported_keys()
                    .map(|keys| keys.iter().count())
                    .unwrap_or(0);
                let axes = evdev
                    .supported_absolute_axes()
                    .map(|axes| axes.iter().count())
                    .unwrap_or(0);

                rows.extend([
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
                    ("Absolute Axes", axes.to_string()),
                ]);

                None
            }
            Err(err) => {
                rows.push((
                    "Cursor Capable",
                    if device.supports_cursor_control {
                        "yes (from last scan)".to_string()
                    } else {
                        "no".to_string()
                    },
                ));
                Some(err.to_string())
            }
        };

        Self { rows, error, title }
    }
}

pub fn render_device_management_screen(
    frame: &mut Frame,
    area: Rect,
    devices: &[DeviceInfo],
    table_state: &mut TableState,
    details: Option<&DeviceDetails>,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    render_device_list(frame, layout[0], devices, table_state);
    render_device_details(frame, layout[1], details);
}

fn render_device_list(
    frame: &mut Frame,
    area: Rect,
    devices: &[DeviceInfo],
    table_state: &mut TableState,
) {
    let header = Row::new(
        [" Cursor", "Device ID", "Name", "evdev Path"]
            .into_iter()
            .map(|title| Cell::from(title).fg(Color::White).bold()),
    )
    .style(Style::default().bg(Color::Black))
    .height(1);

    let data_rows = devices.iter().map(|device| {
        let (marker, marker_color) = if device.supports_cursor_control {
            ("  ✓", Color::LightGreen)
        } else {
            ("  ·", Color::DarkGray)
        };

        Row::new(vec![
            Cell::from(marker).fg(marker_color),
            Cell::from(device.product_id.to_string()),
            Cell::from(device.get_device_name()),
            Cell::from(device.evdev_path.clone()),
        ])
    });

    let widths = [
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(16),
        Constraint::Length(20),
    ];

    let table = Table::new(data_rows, widths)
        .block(Block::bordered().title(format!(" Connected Devices ({}) ", devices.len())))
        .header(header)
        .style(Style::default().fg(Color::White))
        .row_highlight_style(Style::default().bg(Color::White).fg(Color::Black))
        .column_spacing(1);

    frame.render_stateful_widget(table, area, table_state);
}

fn render_device_details(frame: &mut Frame, area: Rect, details: Option<&DeviceDetails>) {
    let Some(details) = details else {
        frame.render_widget(
            Paragraph::new(" No device selected.")
                .block(Block::bordered().title(" Device Details "))
                .fg(Color::DarkGray),
            area,
        );
        return;
    };

    let block = Block::bordered().title(format!(" Device Details: [{}] ", details.title));

    // Reserve a line at the bottom for the probe error, when there is one.
    let (table_area, error_area) = match &details.error {
        Some(_) => {
            let inner = block.inner(area);
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(2)])
                .split(inner);
            frame.render_widget(block, area);
            (split[0], Some(split[1]))
        }
        None => {
            let inner = block.inner(area);
            frame.render_widget(block, area);
            (inner, None)
        }
    };

    let detail_rows = details.rows.iter().map(|(label, value)| {
        Row::new(vec![
            Cell::from(format!(" {label}")).fg(Color::Gray).bold(),
            Cell::from(value.as_str()).fg(Color::White),
        ])
    });

    frame.render_widget(
        Table::new(
            detail_rows,
            [Constraint::Percentage(35), Constraint::Percentage(65)],
        )
        .column_spacing(1),
        table_area,
    );

    if let (Some(error_area), Some(error)) = (error_area, &details.error) {
        frame.render_widget(
            Paragraph::new(format!(" ⚠ {error}"))
                .fg(Color::LightRed)
                .wrap(ratatui::widgets::Wrap { trim: true }),
            error_area,
        );
    }
}
