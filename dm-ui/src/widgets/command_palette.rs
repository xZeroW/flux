use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub struct CommandPalette<'a> {
    pub query: &'a str,
    pub commands: &'a [String],
    pub selected: usize,
}

impl Widget for CommandPalette<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.min(72);
        let height = (self.commands.len() as u16 + 5).min(area.height.saturating_sub(2));
        if width < 12 || height < 5 {
            return;
        }
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let block = Block::default()
            .title(" Commands ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup);
        Clear.render(popup, buf);
        block.render(popup, buf);

        let [search_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .areas(inner);
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(self.query),
            Span::styled("▏", Style::default().fg(Color::DarkGray)),
        ]))
        .render(search_area, buf);

        let rows = self
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| {
                let style = if index == self.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {command}"), style))
            })
            .collect::<Vec<_>>();
        Paragraph::new(rows)
            .alignment(Alignment::Left)
            .render(list_area, buf);
    }
}
