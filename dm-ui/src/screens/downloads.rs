use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};
use serde::{Deserialize, Serialize};

pub struct DownloadsScreen<'a> {
    pub focused: bool,
    pub selected_row: usize,
    pub downloads: &'a [DownloadEntry],
    pub progress_bar: bool,
    pub details_tab: DetailsTab,
    pub files_scroll: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailsTab {
    #[default]
    Info,
    Files,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(from = "String", into = "String")]
pub enum DownloadStatus {
    #[default]
    Downloading,
    Paused,
    Complete,
    Failed(String),
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Downloading => f.write_str("Downloading"),
            Self::Paused => f.write_str("Paused"),
            Self::Complete => f.write_str("Complete"),
            Self::Failed(reason) => write!(f, "Failed: {reason}"),
        }
    }
}

impl From<String> for DownloadStatus {
    fn from(text: String) -> Self {
        match text.as_str() {
            "Downloading" => Self::Downloading,
            "Paused" => Self::Paused,
            "Complete" => Self::Complete,
            _ => Self::Failed(
                text.strip_prefix("Failed: ")
                    .map_or(text.clone(), str::to_owned),
            ),
        }
    }
}

impl From<DownloadStatus> for String {
    fn from(status: DownloadStatus) -> Self {
        status.to_string()
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DownloadEntry {
    pub name: String,
    pub provider: String,
    pub received: u64,
    pub total: Option<u64>,
    pub speed: String,
    pub status: DownloadStatus,
    pub url: String,
    pub destination: String,
    /// Live swarm details; only known while a torrent is running.
    #[serde(skip)]
    pub torrent: Option<TorrentDetails>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TorrentFile {
    pub path: String,
    pub size: u64,
    pub downloaded: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TorrentDetails {
    pub info_hash: String,
    pub seeders: u32,
    pub leechers: u32,
    pub connecting: u32,
    pub queued: u32,
    pub peers_seen: u32,
    pub uploaded_bytes: u64,
    pub upload_speed: u64,
    pub eta: Option<String>,
    pub files: Vec<TorrentFile>,
}

impl DownloadEntry {
    fn file_lines(&self) -> Vec<Line<'static>> {
        let files = self.torrent.as_ref().map_or(&[][..], |t| &t.files[..]);
        files
            .iter()
            .map(|file| {
                let percent = if file.size > 0 {
                    file.downloaded.min(file.size) as f64 * 100.0 / file.size as f64
                } else {
                    100.0
                };
                Line::from(format!(
                    "{} — {} ({percent:.0}%)",
                    file.path,
                    format_bytes(file.size)
                ))
            })
            .collect()
    }

    fn detail_lines(&self, progress_bar: bool) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines = vec![
            Line::from(Span::styled("DETAILS", bold)),
            Line::from(Span::styled(format!("File: {}", self.name), bold)),
            Line::from(format!("URL: {}", self.url)),
            Line::from(format!("Progress: {}", self.progress(progress_bar))),
            Line::from(format!("Destination: {}", self.destination)),
        ];
        let Some(torrent) = &self.torrent else {
            return lines;
        };
        lines.push(Line::from(format!(
            "Seeders: {}   Leechers: {}   Connecting: {}   Queued: {}   Seen: {}",
            torrent.seeders,
            torrent.leechers,
            torrent.connecting,
            torrent.queued,
            torrent.peers_seen
        )));
        lines.push(Line::from(format!(
            "Uploaded: {} ({}/s)   ETA: {}",
            format_bytes(torrent.uploaded_bytes),
            format_bytes(torrent.upload_speed),
            torrent.eta.as_deref().unwrap_or("—")
        )));
        if !torrent.info_hash.is_empty() {
            lines.push(Line::from(format!("Info hash: {}", torrent.info_hash)));
        }
        lines
    }

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

pub fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{bytes} B")
    }
}

impl Widget for DownloadsScreen<'_> {
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

        let selected = self.downloads.get(self.selected_row);
        let is_torrent = selected.is_some_and(|download| download.torrent.is_some());
        let details_height = if is_torrent { 10 } else { 7 };

        let [downloads_area, divider_area, details_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(1),
                Constraint::Length(details_height),
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
                download.status.to_string(),
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

        let Some(download) = selected else {
            Paragraph::new("No downloads yet").render(details_area, buf);
            return;
        };
        if !is_torrent {
            Paragraph::new(download.detail_lines(self.progress_bar)).render(details_area, buf);
            return;
        }

        let file_count = download.torrent.as_ref().map_or(0, |t| t.files.len());
        let active = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let tab_style = |tab| {
            if self.details_tab == tab {
                active
            } else {
                Style::default().fg(Color::DarkGray)
            }
        };
        let tabs = Line::from(vec![
            Span::styled("Info", tab_style(DetailsTab::Info)),
            Span::raw("  "),
            Span::styled(
                format!("Files ({file_count})"),
                tab_style(DetailsTab::Files),
            ),
            Span::styled("   [t] switch tab", Style::default().fg(Color::DarkGray)),
        ]);
        let [tabs_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .areas(details_area);
        Paragraph::new(tabs).render(tabs_area, buf);

        match self.details_tab {
            DetailsTab::Info => {
                Paragraph::new(download.detail_lines(self.progress_bar)).render(body_area, buf);
            }
            DetailsTab::Files => {
                let lines = download.file_lines();
                if lines.is_empty() {
                    Paragraph::new("Waiting for torrent metadata…").render(body_area, buf);
                    return;
                }
                let visible = (body_area.height as usize).saturating_sub(1).max(1);
                let start = self.files_scroll.min(lines.len().saturating_sub(visible));
                let end = (start + visible).min(lines.len());
                let mut shown: Vec<Line> = lines[start..end].to_vec();
                shown.push(Line::from(Span::styled(
                    format!(
                        "{}-{} of {}  [PgUp/PgDn or J/K] scroll",
                        start + 1,
                        end,
                        lines.len()
                    ),
                    Style::default().fg(Color::DarkGray),
                )));
                Paragraph::new(shown).render(body_area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_its_text_form() {
        for status in [
            DownloadStatus::Downloading,
            DownloadStatus::Paused,
            DownloadStatus::Complete,
            DownloadStatus::Failed("connection reset".to_owned()),
        ] {
            assert_eq!(DownloadStatus::from(String::from(status.clone())), status);
        }
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GB");
    }
}
