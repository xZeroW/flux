use crate::{
    downloads::DownloadManager,
    settings::{ProgressDisplay, Settings},
};
use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use dm_providers::ProviderCommand;
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
                    downloads: self.downloads.entries().to_vec(),
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
        if self.input_popup.is_some() {
            self.handle_input_key(key_event.code);
            return Ok(());
        }
        if self.command_palette_open {
            self.handle_command_palette_key(key_event.code);
            return Ok(());
        }

        match key_event.code {
            KeyCode::Char(':') => {
                self.command_palette_open = true;
                self.command_query.clear();
                self.selected_command = 0;
            }
            KeyCode::Char('q' | 'Q') => self.exit(),
            KeyCode::Char('p' | 'P')
                if matches!(self.current_screen, Screen::Downloads)
                    && self.focus == Focus::Content =>
            {
                self.downloads.toggle_pause(self.selected_download);
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Sidebar,
                };
            }
            KeyCode::Char('h') | KeyCode::Left => self.focus = Focus::Sidebar,
            KeyCode::Char('l') | KeyCode::Right => self.focus = Focus::Content,
            KeyCode::Char('j') | KeyCode::Down if self.focus == Focus::Sidebar => {
                self.sidebar_selection = self
                    .sidebar_selection
                    .step(true, dm_providers::providers().len());
            }
            KeyCode::Char('k') | KeyCode::Up if self.focus == Focus::Sidebar => {
                self.sidebar_selection = self
                    .sidebar_selection
                    .step(false, dm_providers::providers().len());
            }
            KeyCode::Char('j') | KeyCode::Down
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Downloads) =>
            {
                self.selected_download = (self.selected_download + 1)
                    .min(self.downloads.entries().len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Downloads) =>
            {
                self.selected_download = self.selected_download.saturating_sub(1);
            }
            KeyCode::Char('j') | KeyCode::Down
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Settings) =>
            {
                self.selected_setting = (self.selected_setting + 1).min(Settings::COUNT - 1);
            }
            KeyCode::Char('k') | KeyCode::Up
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Settings) =>
            {
                self.selected_setting = self.selected_setting.saturating_sub(1);
            }
            KeyCode::Enter if self.focus == Focus::Sidebar => self.activate_sidebar_selection(),
            KeyCode::Enter | KeyCode::Char(' ')
                if self.focus == Focus::Content
                    && matches!(self.current_screen, Screen::Settings) =>
            {
                self.settings.toggle(self.selected_setting)?;
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

    fn handle_command_palette_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => self.command_palette_open = false,
            KeyCode::Enter => {
                let commands = self.filtered_commands();
                self.command_palette_open = false;
                if let Some(command) = commands.get(self.selected_command) {
                    self.input_popup = Some(ProviderInput {
                        provider_id: command.provider_id,
                        title: command.input_prompt.to_owned(),
                        value: String::new(),
                    });
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_command = self.selected_command.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let count = self.filtered_commands().len();
                if count > 0 {
                    self.selected_command = (self.selected_command + 1).min(count - 1);
                }
            }
            KeyCode::Backspace => {
                self.command_query.pop();
                self.selected_command = 0;
            }
            KeyCode::Char(character) if !character.is_control() => {
                self.command_query.push(character);
                self.selected_command = 0;
            }
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyCode) {
        let Some(input) = self.input_popup.as_mut() else {
            return;
        };
        match key {
            KeyCode::Esc => self.input_popup = None,
            KeyCode::Enter => {
                if let Some(input) = self.input_popup.take() {
                    self.downloads
                        .start_download(input.provider_id, input.value);
                    self.selected_download = self.downloads.entries().len().saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                input.value.pop();
            }
            KeyCode::Char(character) if !character.is_control() => {
                input.value.push(character);
            }
            _ => {}
        }
    }

    fn filtered_commands(&self) -> Vec<ProviderCommand> {
        dm_providers::commands_matching(&self.command_query)
    }

    fn activate_sidebar_selection(&mut self) {
        match self.sidebar_selection {
            SidebarItem::Downloads => self.current_screen = Screen::Downloads,
            SidebarItem::Settings => self.current_screen = Screen::Settings,
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
