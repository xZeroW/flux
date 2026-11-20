use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Widget},
};

pub struct SettingsScreen {
    pub skip_splash: bool,
    pub progress_bar: bool,
    pub selected_setting: usize,
    pub focused: bool,
}

impl Widget for SettingsScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let checkbox = if self.skip_splash { "[x]" } else { "[ ]" };
        let progress_mode = if self.progress_bar {
            "Progress bar"
        } else {
            "Text"
        };
        let progress_label = format!(" Download progress: {progress_mode}");
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let settings = Paragraph::new(vec![
            Line::from(Span::styled(
                "GENERAL",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            setting_line(
                0,
                self.selected_setting,
                checkbox,
                " Skip splash screen on startup",
            ),
            Line::from(""),
            setting_line(1, self.selected_setting, "[x]", &progress_label),
            Line::from(""),
            Line::from(Span::styled(
                "Use Up/Down to select; Enter or Space to toggle",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Left)
        .block(
            Block::bordered()
                .title(" Settings ")
                .padding(Padding::horizontal(1))
                .border_style(border_style),
        );
        settings.render(area, buf);
    }
}

fn setting_line<'a>(index: usize, selected: usize, checkbox: &'a str, label: &'a str) -> Line<'a> {
    let style = if index == selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::styled(checkbox, style),
        Span::styled(label, style),
    ])
}
