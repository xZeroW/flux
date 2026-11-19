use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use dm_ui::screens::{Screen, downloads::DownloadsScreen, splash::SplashScreen};
use dm_ui::widgets::{
    footer::Footer,
    sidebar::{Sidebar, SidebarItem},
};
use futures_util::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
};
use std::time::{Duration, Instant};

pub struct App {
    current_screen: Screen,
    exit: bool,
    splash_started: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            current_screen: Screen::default(),
            exit: false,
            splash_started: Instant::now(),
        }
    }
}

impl App {
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();
        let mut animation_tick = tokio::time::interval(Duration::from_millis(50));

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                event = events.next() => self.handle_event(event.transpose()?)?,
                _ = animation_tick.tick(), if matches!(&self.current_screen, Screen::Splash) => {
                    if self.splash_started.elapsed() >= Duration::from_secs(2) {
                        self.current_screen = Screen::Downloads;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        match &self.current_screen {
            Screen::Splash => frame.render_widget(
                SplashScreen::new(self.splash_started.elapsed().as_secs_f32()),
                frame.area(),
            ),
            Screen::Downloads => {
                let [content_area, footer_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(5), Constraint::Length(3)])
                    .areas(frame.area());
                let [sidebar_area, main_area] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(26), Constraint::Min(0)])
                    .areas(content_area);

                frame.render_widget(Sidebar::new(SidebarItem::Downloads), sidebar_area);
                frame.render_widget(DownloadsScreen, main_area);
                frame.render_widget(Footer, footer_area);
            }
        }
    }

    fn handle_event(&mut self, event: Option<Event>) -> Result<()> {
        let Some(event) = event else {
            self.exit = true;
            return Ok(());
        };

        match event {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_pressed(key_event)
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_pressed(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
