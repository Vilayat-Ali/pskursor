use ratatui::{
    Frame, Terminal,
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Paragraph, Tabs},
};
use std::{
    error,
    io::{self, stdout},
    thread,
    time::Duration,
};
use termion::{event::Key, input::TermRead, raw::IntoRawMode, screen::IntoAlternateScreen};

const MENU_TITLES: [&str; 2] = ["Device Management (d)", "Device Stats (s)"];

pub fn setup_tui() -> Result<(), Box<dyn error::Error>> {
    let stdout = stdout().into_raw_mode()?.into_alternate_screen()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut active_tab = 0;

    terminal.clear()?;

    let mut keys = io::stdin().keys();

    loop {
        terminal.draw(|frame| {
            let area = frame.area();

            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(9),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(area);

            render_banner(frame, main_layout[0]);
            render_navbar(frame, main_layout[1], active_tab);
            render_content(frame, main_layout[2], active_tab);
        })?;

        if let Some(Ok(key)) = keys.next() {
            match key {
                Key::Char('q') | Key::Char('c') => break,
                Key::Char('d') => active_tab = 0,
                Key::Char('s') => active_tab = 1,
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(16));
    }

    Ok(())
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
            Constraint::Length(9),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let banner = Paragraph::new(banner_text)
        .alignment(Alignment::Center)
        .red();
    let subtitle = Paragraph::new("Made with ❤️ + 🦀 by Vilayat")
        .alignment(Alignment::Center)
        .white();

    frame.render_widget(banner, layout[0]);
    frame.render_widget(subtitle, layout[2]);
}

fn render_navbar(frame: &mut Frame, area: Rect, active_tab: usize) {
    let main_block = Block::bordered().title(" Menu (m) ").fg(Color::Green);

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

fn render_content(frame: &mut Frame, area: Rect, selected_tab: usize) {
    let text = match selected_tab {
        0 => "Great terminal interfaces start with a single widget.".into(),
        1 => "In the terminal, we don't just render widgets; we create dreams.".into(),
        2 => "Render boldly, style with purpose.".bold(),
        _ => unreachable!(),
    };
    let block = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::bordered());

    frame.render_widget(block, area);
}
