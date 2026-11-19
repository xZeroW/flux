use crate::ascii_art::FLUX_LOGO;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};

pub struct SplashScreen {
    elapsed: f32,
}

impl SplashScreen {
    pub fn new(elapsed: f32) -> Self {
        Self { elapsed }
    }
}

impl Widget for SplashScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [_, content_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .areas(area);

        let mut lines = FLUX_LOGO
            .lines()
            .map(|line| {
                let spans = line
                    .chars()
                    .enumerate()
                    .map(|(x, ch)| {
                        let wave = (x as f32 * 0.25 - self.elapsed * 3.0).sin();
                        let intensity = (wave + 1.0) / 2.0;
                        let color = Color::Rgb(
                            (100.0 + 100.0 * intensity) as u8,
                            (50.0 + 80.0 * intensity) as u8,
                            (180.0 + 75.0 * intensity) as u8,
                        );
                        Span::styled(ch.to_string(), Style::default().fg(color))
                    })
                    .collect::<Vec<_>>();
                Line::from(spans)
            })
            .collect::<Vec<_>>();
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "TUI DOWNLOAD MANAGER",
            Style::default().fg(Color::Cyan),
        )));

        Paragraph::new(Text::from(lines))
            .centered()
            .render(content_area, buf);
    }
}
