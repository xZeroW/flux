use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Row, Table, Widget},
};

pub struct DownloadsScreen;

impl Widget for DownloadsScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(" Downloads ")
            .padding(Padding::horizontal(1));
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

        let header = Row::new(["Name", "Provider", "Progress", "Speed"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1);
        let rows = [
            Row::new(["Ubuntu 26.04.iso", "Torrent", "█████░░░", "12 MB/s"]),
            Row::new(["archive.zip", "MediaFire", "███░░░░░", "4 MB/s"]),
            Row::new(["video.mp4", "MEGA", "███████░", "8 MB/s"]),
        ];
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(38),
                Constraint::Percentage(22),
                Constraint::Percentage(24),
                Constraint::Percentage(16),
            ],
        )
        .header(header)
        .row_highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        table.render(downloads_area, buf);

        Paragraph::new(Line::from("─".repeat(divider_area.width as usize)))
            .style(Style::default().fg(Color::DarkGray))
            .render(divider_area, buf);

        let details = Paragraph::new(vec![
            Line::from(Span::styled(
                "DETAILS",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "File: archive.zip",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from("URL: mediafire.com/..."),
            Line::from("Size: 2.4 GB"),
            Line::from("Destination: ~/Downloads"),
        ]);
        details.render(details_area, buf);
    }
}
