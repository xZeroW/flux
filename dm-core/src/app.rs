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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sidebar,
    Content,
}

pub struct App {
    current_screen: Screen,
    sidebar_selection: SidebarItem,
    focus: Focus,
    selected_download: usize,
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
            sidebar_selection: SidebarItem::Downloads,
            focus: Focus::Content,
            selected_download: 0,
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

        frame.render_widget(
            Sidebar::new(self.sidebar_selection, self.focus == Focus::Sidebar),
            sidebar_area,
        );
        match &self.current_screen {
            Screen::Downloads => frame.render_widget(
                DownloadsScreen {
                    focused: self.focus == Focus::Content,
                    selected_row: self.selected_download,
                },
                main_area,
            ),
            Screen::Settings => frame.render_widget(
                SettingsScreen {
                    skip_splash: self.settings.skip_splash,
                    focused: self.focus == Focus::Content,
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
        match key_event.code {
            KeyCode::Char('q' | 'Q') => self.exit(),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Sidebar,
                };
            }
            KeyCode::Char('h') | KeyCode::Left => self.focus = Focus::Sidebar,
            KeyCode::Char('l') | KeyCode::Right => self.focus = Focus::Content,
            KeyCode::Char('j') | KeyCode::Down if self.focus == Focus::Sidebar => {
                self.move_sidebar_selection(true);
            }
            KeyCode::Char('k') | KeyCode::Up if self.focus == Focus::Sidebar => {
                self.move_sidebar_selection(false);
            }
            KeyCode::Char('j') | KeyCode::Down
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Downloads) =>
            {
                self.selected_download = (self.selected_download + 1).min(2);
            }
            KeyCode::Char('k') | KeyCode::Up
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Downloads) =>
            {
                self.selected_download = self.selected_download.saturating_sub(1);
            }
            KeyCode::Enter if self.focus == Focus::Sidebar => self.activate_sidebar_selection(),
            KeyCode::Enter | KeyCode::Char(' ')
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Settings) =>
            {
                self.toggle_skip_splash()?;
            }
            KeyCode::Char('s' | 'S') => {
                self.current_screen = Screen::Settings;
                self.sidebar_selection = SidebarItem::Settings;
                self.focus = Focus::Content;
            }
            KeyCode::Esc if matches!(self.current_screen, Screen::Settings) => {
                self.current_screen = Screen::Downloads;
                self.sidebar_selection = SidebarItem::Downloads;
            }
            KeyCode::Enter if matches!(self.current_screen, Screen::Splash) => {
                self.current_screen = Screen::Downloads;
                self.sidebar_selection = SidebarItem::Downloads;
            }
            _ => {}
        }
        Ok(())
    }

    fn move_sidebar_selection(&mut self, forward: bool) {
        const ITEMS: [SidebarItem; 8] = [
            SidebarItem::Downloads,
            SidebarItem::History,
            SidebarItem::Queue,
            SidebarItem::Settings,
            SidebarItem::MediaFire,
            SidebarItem::Mega,
            SidebarItem::GoogleDrive,
            SidebarItem::Torrents,
        ];
        let index = ITEMS
            .iter()
            .position(|item| *item == self.sidebar_selection)
            .unwrap_or(0);
        let next_index = if forward {
            (index + 1) % ITEMS.len()
        } else {
            (index + ITEMS.len() - 1) % ITEMS.len()
        };
        self.sidebar_selection = ITEMS[next_index];
    }

    fn activate_sidebar_selection(&mut self) {
        match self.sidebar_selection {
            SidebarItem::Downloads => self.current_screen = Screen::Downloads,
            SidebarItem::Settings => self.current_screen = Screen::Settings,
            _ => {}
        }
    }

    fn toggle_skip_splash(&mut self) -> Result<()> {
        self.settings.skip_splash = !self.settings.skip_splash;
        self.settings.save()
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
