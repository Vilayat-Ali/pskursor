use ratatui::{Frame, layout::Rect, widgets::Block};

pub fn render_device_stats_screen(frame: &mut Frame, area: Rect) {
    let block = Block::bordered().title(" Device Stats ");

    frame.render_widget(block, area);
}
