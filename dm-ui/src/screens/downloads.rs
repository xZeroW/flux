use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

pub struct DownloadsScreen {
    pub focused: bool,
    pub selected_row: usize,
    pub downloads: Vec<DownloadEntry>,
    pub progress_bar: bool,
}

#[derive(Clone)]
pub struct DownloadEntry {
    pub name: String,
    pub provider: String,
    pub received: u64,
    pub total: Option<u64>,
    pub speed: String,
    pub status: String,
    pub url: String,
    pub destination: String,
}

impl DownloadEntry {
    fn progress(&self, progress_bar: bool) -> String {
        if progress_bar {
            if let Some(total) = self.total.filter(|total| *total > 0) {
                let progress = self.received.min(total) as f64 / total as f64 * 8.0;
                let filled = progress.round() as usize;
                return format!(
                    "{}{} {:.0}%",
                    "█".repeat(filled),
                    "░".repeat(8 - filled),
                    progress / 8.0 * 100.0
                );
            }
            return format!("████░░░░ {}", self.status);
        }
        match self.total {
            Some(total) if total > 0 => format!(
                "{:.0}% ({}/{})",
                self.received as f64 * 100.0 / total as f64,
                format_bytes(self.received),
                format_bytes(total)
            ),
            _ => format!("{} — {}", format_bytes(self.received), self.status),
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{bytes} B")
    }
}

impl Widget for DownloadsScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let block = Block::bordered()
            .title(" Downloads ")
            .padding(Padding::horizontal(1))
            .border_style(border_style);
        let inner = block.inner(area);
        block.render(area, buf);

        let [downloads_area, divider_area, details_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(1),
                Constraint::Min(7),
            ])
            .areas(inner);

        let header = Row::new(["Name", "Provider", "Progress", "Speed", "Status"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1);
        let rows = self.downloads.iter().map(|download| {
            Row::new([
                download.name.clone(),
                download.provider.clone(),
                download.progress(self.progress_bar),
                download.speed.clone(),
                download.status.clone(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(32),
                Constraint::Percentage(18),
                Constraint::Percentage(24),
                Constraint::Percentage(14),
                Constraint::Percentage(12),
            ],
        )
        .header(header)
        .row_highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        let mut table_state = TableState::default().with_selected(Some(self.selected_row));
        StatefulWidget::render(table, downloads_area, buf, &mut table_state);

        Paragraph::new(Line::from("─".repeat(divider_area.width as usize)))
            .style(Style::default().fg(Color::DarkGray))
            .render(divider_area, buf);

        let details = self.downloads.get(self.selected_row).map_or_else(
            || Paragraph::new("No downloads yet"),
            |download| {
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "DETAILS",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        format!("File: {}", download.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!("URL: {}", download.url)),
                    Line::from(format!(
                        "Progress: {}",
                        download.progress(self.progress_bar)
                    )),
                    Line::from(format!("Destination: {}", download.destination)),
                ])
            },
        );
        details.render(details_area, buf);
    }
}
