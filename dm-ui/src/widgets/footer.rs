use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

pub struct Footer;

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("[a]", Style::default().fg(Color::Cyan)),
            Span::raw(" Add download   "),
            Span::styled("[Space]", Style::default().fg(Color::Cyan)),
            Span::raw(" Pause/Resume   "),
            Span::styled("[d]", Style::default().fg(Color::Cyan)),
            Span::raw(" Details   "),
            Span::styled("[?]", Style::default().fg(Color::Cyan)),
            Span::raw(" Help   "),
            Span::styled("[q]", Style::default().fg(Color::Cyan)),
            Span::raw(" Quit"),
        ]))
        .block(Block::bordered());
        footer.render(area, buf);
    }
}
