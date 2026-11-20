use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

pub struct Footer<'a> {
    pub message: Option<&'a str>,
}

impl Widget for Footer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let content = self.message.map_or_else(
            || {
                Line::from(vec![
                    Span::styled("[?]", Style::default().fg(Color::Cyan)),
                    Span::raw(" Help   "),
                    Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                    Span::raw(" Focus   "),
                    Span::styled("[h/j/k/l]", Style::default().fg(Color::Cyan)),
                    Span::raw(" Navigate   "),
                    Span::styled("[:]", Style::default().fg(Color::Cyan)),
                    Span::raw(" Commands   "),
                    Span::styled("[q]", Style::default().fg(Color::Cyan)),
                    Span::raw(" Quit"),
                ])
            },
            |message| Line::from(message),
        );
        let footer = Paragraph::new(content).centered().block(Block::bordered());
        footer.render(area, buf);
    }
}
