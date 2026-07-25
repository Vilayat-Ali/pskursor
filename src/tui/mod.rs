pub mod screens;

use std::{io::stdout, thread, time::Duration};

use ratatui::{
    Frame, Terminal,
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Paragraph, TableState, Tabs},
};
use termion::{
    async_stdin, event::Key, input::TermRead, raw::IntoRawMode, screen::IntoAlternateScreen,
};

use crate::{
    device::{DeviceInfo, get_devices},
    error::Result,
    tui::screens::{
        control::render_control_help,
        device_management::{DeviceDetails, render_device_management_screen},
        device_stats::render_device_stats_screen,
    },
};

const MENU_TITLES: [&str; 3] = ["Device Management (d)", "Device Stats (s)", "Controls (k)"];

/// Frame budget. Input is polled without blocking, so this also sets how
/// responsively key presses are picked up.
const TICK: Duration = Duration::from_millis(16);

/// A one-line message shown under the content area.
struct Status {
    text: String,
    is_error: bool,
}

/// TUI state, kept in one place so key handling and rendering cannot disagree.
struct App {
    devices: Vec<DeviceInfo>,
    table_state: TableState,
    /// evdev metadata for the highlighted device, refreshed only when the
    /// selection changes — opening the node every frame would hammer the
    /// device file for no benefit.
    details: Option<DeviceDetails>,
    active_tab: usize,
    status: Option<Status>,
}

impl App {
    fn new() -> Result<Self> {
        let devices = get_devices()?;
        let mut table_state = TableState::default();
        table_state.select((!devices.is_empty()).then_some(0));

        let mut app = Self {
            devices,
            table_state,
            details: None,
            active_tab: 0,
            status: None,
        };
        app.refresh_details();

        Ok(app)
    }

    fn selected_device(&self) -> Option<&DeviceInfo> {
        self.table_state
            .selected()
            .and_then(|index| self.devices.get(index))
    }

    fn refresh_details(&mut self) {
        self.details = self.selected_device().map(DeviceDetails::probe);
    }

    fn select(&mut self, index: usize) {
        if self.devices.is_empty() {
            self.table_state.select(None);
            self.details = None;
            return;
        }

        let clamped = index.min(self.devices.len() - 1);
        if self.table_state.selected() != Some(clamped) {
            self.table_state.select(Some(clamped));
            self.refresh_details();
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let current = self.table_state.selected().unwrap_or(0) as isize;
        self.select(current.saturating_add(delta).max(0) as usize);
    }

    fn reload_devices(&mut self) {
        match get_devices() {
            Ok(devices) => {
                let count = devices.len();
                let previous = self.table_state.selected().unwrap_or(0);

                self.devices = devices;
                // Drop the selection first so `select` cannot keep a stale
                // index when the list shrank under it.
                self.table_state.select(None);
                self.select(previous);

                self.set_status(format!("Reloaded — {count} device(s) found"), false);
            }
            Err(err) => self.set_status(format!("Reload failed: {err}"), true),
        }
    }

    fn activate_selected(&mut self) {
        let Some(device) = self.selected_device() else {
            self.set_status("No device selected".to_string(), true);
            return;
        };

        let name = device.get_device_name();

        if !device.supports_cursor_control {
            self.set_status(
                format!("{name} is not a gamepad — it cannot drive the cursor"),
                true,
            );
            return;
        }

        match device.activate() {
            Ok(path) => self.set_status(format!("Activated {name} → {}", path.display()), false),
            Err(err) => self.set_status(format!("Activation failed: {err}"), true),
        }
    }

    fn set_status(&mut self, text: String, is_error: bool) {
        self.status = Some(Status { text, is_error });
    }
}

/// Run the configuration TUI until the user quits.
pub fn setup_tui() -> Result<()> {
    // Both guards restore the terminal when dropped, including on early return.
    let screen = stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut terminal = Terminal::new(TermionBackend::new(screen))?;
    terminal.clear()?;

    let mut app = App::new()?;
    let mut keys = async_stdin().keys();

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        while let Some(key) = keys.next().transpose()? {
            match key {
                Key::Char('q') | Key::Esc | Key::Ctrl('c') => return Ok(()),
                Key::Char('d') => app.active_tab = 0,
                Key::Char('s') => app.active_tab = 1,
                Key::Char('k') => app.active_tab = 2,
                Key::Char('y') => app.activate_selected(),
                Key::Char('r') => app.reload_devices(),
                Key::Up => app.move_selection(-1),
                Key::Down => app.move_selection(1),
                _ => {}
            }
        }

        thread::sleep(TICK);
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_banner(frame, main_layout[0]);
    render_navbar(frame, main_layout[1], app.active_tab);
    render_content(frame, main_layout[2], app);
    render_status(frame, main_layout[3], app.status.as_ref());
}

fn render_banner(frame: &mut Frame, area: Rect) {
    let banner_text = r#"
 ██████   ██████  ██   ██ ██    ██ ██████   ██████   ██████  ██████
 ██   ██ ██       ██  ██  ██    ██ ██   ██ ██       ██    ██ ██   ██
 ██████   █████   █████   ██    ██ ██████   █████   ██    ██ ██████
 ██           ██  ██  ██  ██    ██ ██   ██      ██  ██    ██ ██   ██
 ██      ██████   ██   ██  ██████  ██   ██ ██████    ██████  ██   ██
"#;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(banner_text)
            .alignment(Alignment::Center)
            .red(),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new("Made with ❤️ + 🦀 by Vilayat")
            .alignment(Alignment::Center)
            .white(),
        layout[1],
    );
}

fn render_navbar(frame: &mut Frame, area: Rect, active_tab: usize) {
    let main_block = Block::bordered().title(" Menu ").fg(Color::Green);
    let inner_area = main_block.inner(area);
    frame.render_widget(main_block, area);

    let tabs = Tabs::new(MENU_TITLES)
        .select(active_tab)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, inner_area);
}

fn render_content(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.active_tab {
        1 => render_device_stats_screen(frame, area),
        2 => render_control_help(frame, area),
        // Device management is the landing tab and the fallback for any
        // out-of-range index.
        _ => render_device_management_screen(
            frame,
            area,
            &app.devices,
            &mut app.table_state,
            app.details.as_ref(),
        ),
    }
}

fn render_status(frame: &mut Frame, area: Rect, status: Option<&Status>) {
    let (text, color) = match status {
        Some(status) if status.is_error => (status.text.as_str(), Color::LightRed),
        Some(status) => (status.text.as_str(), Color::LightGreen),
        None => (
            "q quit · d devices · s stats · k controls · ▲▼ move · y activate · r reload",
            Color::DarkGray,
        ),
    };

    frame.render_widget(Paragraph::new(format!(" {text}")).fg(color), area);
}
