use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

pub struct SplashScreen {}

impl Default for SplashScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl SplashScreen {
    pub fn new() -> Self {
        Self {}
    }
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Length(12),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Percentage(25),
            ])
            .split(area);
    }
}
