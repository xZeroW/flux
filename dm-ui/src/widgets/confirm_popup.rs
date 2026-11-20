use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

pub struct ConfirmPopup<'a> {
    pub title: &'a str,
    pub message: &'a str,
}

impl Widget for ConfirmPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.min(64);
        let height = 7;
        if width < 12 || area.height < height + 2 {
            return;
        }
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));
        let inner = block.inner(popup);
        Clear.render(popup, buf);
        block.render(popup, buf);
        let key = |text: &'static str, color: Color| {
            Span::styled(
                text,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        };
        Paragraph::new(vec![
            Line::from(self.message),
            Line::from(""),
            Line::from(vec![
                key("[y/Enter]", Color::Red),
                Span::raw(" Delete   "),
                key("[n/Esc]", Color::Cyan),
                Span::raw(" Cancel"),
            ]),
        ])
        .wrap(Wrap { trim: true })
        .render(inner, buf);
    }
}
