use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub struct PickerRow {
    pub name: String,
    pub size: Option<String>,
    pub checked: bool,
}

pub struct FilePicker<'a> {
    pub title: &'a str,
    pub rows: &'a [PickerRow],
    pub selected: usize,
    pub loading: bool,
    pub error: Option<&'a str>,
}

impl Widget for FilePicker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width.saturating_sub(8).min(90);
        let height = area.height.saturating_sub(4).min(24);
        if width < 20 || height < 6 {
            return;
        }
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let checked = self.rows.iter().filter(|row| row.checked).count();
        let block = Block::default()
            .title(format!(
                " {} — {checked}/{} selected ",
                self.title,
                self.rows.len()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup);
        Clear.render(popup, buf);
        block.render(popup, buf);

        let hint = Style::default().fg(Color::DarkGray);
        let mut lines: Vec<Line> = Vec::new();
        if self.loading {
            lines.push(Line::from(Span::styled("Loading…", hint)));
        } else if let Some(error) = self.error {
            lines.push(Line::from(Span::styled(
                error.to_owned(),
                Style::default().fg(Color::Red),
            )));
        } else if self.rows.is_empty() {
            lines.push(Line::from(Span::styled("(no files)", hint)));
        }
        let visible = (inner.height as usize)
            .saturating_sub(lines.len() + 2)
            .max(1);
        let start = self.selected.saturating_sub(visible - 1);
        for (index, row) in self.rows.iter().enumerate().skip(start).take(visible) {
            let is_selected = index == self.selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let marker = if is_selected { "▸ " } else { "  " };
            let check = if row.checked { "[x]" } else { "[ ]" };
            lines.push(Line::from(vec![
                Span::styled(format!("{marker}{check} {}", row.name), style),
                Span::styled(format!("  {}", row.size.clone().unwrap_or_default()), hint),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Space select · a all/none · Enter download · Esc cancel",
            hint,
        )));
        Paragraph::new(lines).render(inner, buf);
    }
}
