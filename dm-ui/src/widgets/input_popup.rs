use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub struct InputPopup<'a> {
    pub title: &'a str,
    pub value: &'a str,
    pub masked: bool,
}

impl Widget for InputPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.min(72);
        let height = 5;
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
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup);
        Clear.render(popup, buf);
        block.render(popup, buf);
        Paragraph::new(vec![
            Line::from("Enter a value, then press Enter"),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(if self.masked {
                    "•".repeat(self.value.chars().count())
                } else {
                    self.value.to_owned()
                }),
                Span::styled("▏", Style::default().fg(Color::DarkGray)),
            ]),
        ])
        .render(inner, buf);
    }
}
