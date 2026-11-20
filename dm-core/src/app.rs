use crate::settings::Settings;
use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use dm_ui::screens::{
    Screen, downloads::DownloadsScreen, settings::SettingsScreen, splash::SplashScreen,
};
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
    settings: Settings,
}

impl App {
    pub fn new() -> Result<Self> {
        let settings = Settings::load()?;
        let current_screen = if settings.skip_splash {
            Screen::Downloads
        } else {
            Screen::Splash
        };
        Ok(Self {
            current_screen,
            exit: false,
            splash_started: Instant::now(),
            settings,
        })
    }

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
            Screen::Downloads | Screen::Settings => self.draw_main_layout(frame),
        }
    }

    fn draw_main_layout(&self, frame: &mut Frame) {
        let [content_area, footer_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .areas(frame.area());
        let [sidebar_area, main_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(26), Constraint::Min(0)])
            .areas(content_area);

        let selected = match &self.current_screen {
            Screen::Downloads => SidebarItem::Downloads,
            Screen::Settings => SidebarItem::Settings,
            Screen::Splash => unreachable!("splash is drawn without the main layout"),
        };
        frame.render_widget(Sidebar::new(selected), sidebar_area);
        match &self.current_screen {
            Screen::Downloads => frame.render_widget(DownloadsScreen, main_area),
            Screen::Settings => frame.render_widget(
                SettingsScreen {
                    skip_splash: self.settings.skip_splash,
                },
                main_area,
            ),
            Screen::Splash => unreachable!("splash is drawn without the main layout"),
        }
        frame.render_widget(Footer, footer_area);
    }

    fn handle_event(&mut self, event: Option<Event>) -> Result<()> {
        let Some(event) = event else {
            self.exit = true;
            return Ok(());
        };

        match event {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_pressed(key_event)?
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_pressed(&mut self, key_event: KeyEvent) -> Result<()> {
        match (&self.current_screen, key_event.code) {
            (_, KeyCode::Char('q' | 'Q')) => self.exit(),
            (Screen::Splash, KeyCode::Enter) => self.current_screen = Screen::Downloads,
            (Screen::Downloads, KeyCode::Char('s' | 'S')) => self.current_screen = Screen::Settings,
            (Screen::Settings, KeyCode::Esc) => self.current_screen = Screen::Downloads,
            (Screen::Settings, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.settings.skip_splash = !self.settings.skip_splash;
                self.settings.save()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
