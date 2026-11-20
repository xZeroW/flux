use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Widget},
};

pub struct SettingsScreen {
    pub skip_splash: bool,
}

impl Widget for SettingsScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let checkbox = if self.skip_splash { "[x]" } else { "[ ]" };
        let settings = Paragraph::new(vec![
            Line::from(Span::styled(
                "GENERAL",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(checkbox, Style::default().fg(Color::Cyan)),
                Span::raw(" Skip splash screen on startup"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press Space or Enter to toggle",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Left)
        .block(
            Block::bordered()
                .title(" Settings ")
                .padding(Padding::horizontal(1)),
        );
        settings.render(area, buf);
    }
}
