use crate::ascii_art::FLUX_LOGO;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget},
};

pub struct SplashScreen;

impl Widget for SplashScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [_, content_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .areas(area);

        Paragraph::new(format!("{FLUX_LOGO}\nTUI DOWNLOAD MANAGER"))
            .centered()
            .style(Style::default().fg(Color::Cyan))
            .render(content_area, buf);
    }
}
