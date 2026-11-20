use crate::{
    downloads::DownloadManager,
    settings::{ProgressDisplay, Settings},
};
use anyhow::Result;
use crossterm::event::EventStream;
use dm_ui::screens::{
    Screen, downloads::DownloadsScreen, settings::SettingsScreen, splash::SplashScreen,
};
use dm_ui::widgets::{
    command_palette::CommandPalette,
    footer::Footer,
    input_popup::InputPopup,
    sidebar::{Sidebar, SidebarItem},
};
use futures_util::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
};
use std::time::{Duration, Instant};

mod input;

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
    selected_setting: usize,
    exit: bool,
    splash_started: Instant,
    settings: Settings,
    command_palette_open: bool,
    command_query: String,
    selected_command: usize,
    input_popup: Option<ProviderInput>,
    downloads: DownloadManager,
}

struct ProviderInput {
    provider_id: &'static str,
    title: String,
    value: String,
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
            selected_setting: 0,
            exit: false,
            splash_started: Instant::now(),
            settings,
            command_palette_open: false,
            command_query: String::new(),
            selected_command: 0,
            input_popup: None,
            downloads: DownloadManager::new()?,
        })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();
        let mut animation_tick = tokio::time::interval(Duration::from_millis(50));

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                event = events.next() => self.handle_event(event.transpose()?)?,
                Some(()) = self.downloads.recv_event() => {},
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
            .constraints([Constraint::Percentage(14), Constraint::Min(10)])
            .areas(content_area);

        let provider_names: Vec<&str> = dm_providers::providers()
            .iter()
            .map(|provider| provider.name())
            .collect();
        frame.render_widget(
            Sidebar::new(
                self.sidebar_selection,
                self.focus == Focus::Sidebar,
                &provider_names,
            ),
            sidebar_area,
        );
        match &self.current_screen {
            Screen::Downloads => frame.render_widget(
                DownloadsScreen {
                    focused: self.focus == Focus::Content,
                    selected_row: self.selected_download,
                    downloads: self.downloads.entries(),
                    progress_bar: self.settings.progress_display == ProgressDisplay::Bar,
                },
                main_area,
            ),
            Screen::Settings => frame.render_widget(
                SettingsScreen {
                    skip_splash: self.settings.skip_splash,
                    progress_bar: self.settings.progress_display == ProgressDisplay::Bar,
                    selected_setting: self.selected_setting,
                    focused: self.focus == Focus::Content,
                },
                main_area,
            ),
            Screen::Splash => unreachable!("splash is drawn without the main layout"),
        }
        frame.render_widget(
            Footer {
                message: self.downloads.status(),
            },
            footer_area,
        );
        if self.command_palette_open {
            let commands: Vec<String> = self
                .filtered_commands()
                .iter()
                .map(|command| format!("{} — {}", command.name, command.description))
                .collect();
            frame.render_widget(
                CommandPalette {
                    query: &self.command_query,
                    commands: &commands,
                    selected: self.selected_command,
                },
                frame.area(),
            );
        }
        if let Some(input) = &self.input_popup {
            frame.render_widget(
                InputPopup {
                    title: &input.title,
                    value: &input.value,
                },
                frame.area(),
            );
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
