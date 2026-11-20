use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Widget},
};

pub struct MenuItem {
    pub label: String,
    pub value: String,
}

pub struct FileRow {
    pub name: String,
    pub is_folder: bool,
    pub size: Option<String>,
    pub checked: bool,
}

pub enum ProviderView<'a> {
    Menu {
        items: &'a [MenuItem],
        selected: usize,
    },
    Browse {
        path: String,
        rows: &'a [FileRow],
        selected: usize,
        loading: bool,
        error: Option<&'a str>,
    },
}

pub struct ProviderScreen<'a> {
    pub name: &'a str,
    pub focused: bool,
    pub view: ProviderView<'a>,
}

fn highlight(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

impl Widget for ProviderScreen<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let hint_style = Style::default().fg(Color::DarkGray);
        let mut lines: Vec<Line> = Vec::new();
        match &self.view {
            ProviderView::Menu { items, selected } => {
                for (index, item) in items.iter().enumerate() {
                    let marker = if index == *selected { "▸ " } else { "  " };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{marker}{:<16}", item.label),
                            highlight(index == *selected),
                        ),
                        Span::styled(item.value.clone(), hint_style),
                    ]));
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    "Up/Down to select; Enter to activate",
                    hint_style,
                )));
            }
            ProviderView::Browse {
                path,
                rows,
                selected,
                loading,
                error,
            } => {
                lines.push(Line::from(Span::styled(
                    path.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                if *loading {
                    lines.push(Line::from(Span::styled("Loading…", hint_style)));
                } else if let Some(error) = error {
                    lines.push(Line::from(Span::styled(
                        error.to_string(),
                        Style::default().fg(Color::Red),
                    )));
                } else if rows.is_empty() {
                    lines.push(Line::from(Span::styled("(empty)", hint_style)));
                }
                let visible = (area.height as usize).saturating_sub(6).max(1);
                let start = selected.saturating_sub(visible - 1);
                for (index, row) in rows.iter().enumerate().skip(start).take(visible) {
                    let marker = if index == *selected { "▸ " } else { "  " };
                    let check = match (row.is_folder, row.checked) {
                        (true, _) => "   ",
                        (false, true) => "[x]",
                        (false, false) => "[ ]",
                    };
                    let name = if row.is_folder {
                        format!("{}/", row.name)
                    } else {
                        row.name.clone()
                    };
                    let size = row.size.clone().unwrap_or_default();
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{marker}{check} {name}"),
                            highlight(index == *selected),
                        ),
                        Span::styled(format!("  {size}"), hint_style),
                    ]));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Enter open folder · Backspace up · Space select · g get (download) · Esc back",
                    hint_style,
                )));
            }
        }
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(format!(" {} ", self.name))
                    .padding(Padding::horizontal(1))
                    .border_style(border_style),
            )
            .render(area, buf);
    }
}
