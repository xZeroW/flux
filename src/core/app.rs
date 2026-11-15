use std::{io, time::Instant};

use crate::ui::screens::splash::SplashScreen;
use anyhow::Result;
use ratatui::{Frame, Terminal, backend::CrosstermBackend};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    SplashScreen,
}

pub struct App {
    current_screen: Screen,
    splash_screen: SplashScreen,
}

impl App {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            current_screen: Screen::SplashScreen,
            splash_screen: SplashScreen::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let res = self.run_app(&mut terminal).await;

        res
    }

    pub async fn run_app(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let mut last_tick = Instant::now();
        let tick_rate = std::time::Duration::from_millis(100);

        loop {
            terminal.draw(|f| self.draw(f))?;
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        match self.current_screen {
            Screen::SplashScreen => self.draw_splash(frame),
        }
    }

    fn draw_splash(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.splash_screen.render(frame, area);
    }
}
