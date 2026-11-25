use crate::{
    cloud::CloudManager,
    downloads::DownloadManager,
    settings::{ProgressDisplay, Settings},
};
use anyhow::Result;
use crossterm::event::EventStream;
use dm_ui::screens::{
    Screen,
    downloads::{DetailsTab, DownloadsScreen},
    provider::{FileRow, MenuItem, ProviderScreen, ProviderView},
    settings::SettingsScreen,
    splash::SplashScreen,
};
use dm_ui::widgets::{
    command_palette::CommandPalette,
    confirm_popup::ConfirmPopup,
    file_picker::{FilePicker, PickerRow},
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
    details_tab: DetailsTab,
    files_scroll: usize,
    selected_setting: usize,
    exit: bool,
    splash_started: Instant,
    settings: Settings,
    command_palette_open: bool,
    command_query: String,
    selected_command: usize,
    input_popup: Option<ProviderInput>,
    confirm_delete: Option<usize>,
    downloads: DownloadManager,
    cloud: CloudManager,
    browsing: bool,
    selected_menu: usize,
    selected_file: usize,
}

enum InputTarget {
    Download(&'static str),
    Field(usize, dm_providers::cloud::AccountField),
}

struct ProviderInput {
    target: InputTarget,
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
            details_tab: DetailsTab::default(),
            files_scroll: 0,
            selected_setting: 0,
            exit: false,
            splash_started: Instant::now(),
            settings,
            command_palette_open: false,
            command_query: String::new(),
            selected_command: 0,
            input_popup: None,
            confirm_delete: None,
            downloads: DownloadManager::new()?,
            cloud: CloudManager::new(),
            browsing: false,
            selected_menu: 0,
            selected_file: 0,
        })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();
        let mut status_tick = tokio::time::interval(Duration::from_millis(500));
        let mut animation_tick = tokio::time::interval(Duration::from_millis(50));

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                event = events.next() => self.handle_event(event.transpose()?)?,
                Some(()) = self.downloads.recv_event() => {},
                Some(()) = self.cloud.recv_event() => {
                    let count = self.cloud_file_count();
                    self.selected_file = self.selected_file.min(count.saturating_sub(1));
                },
                // Redraw periodically so expired status messages disappear.
                _ = status_tick.tick() => {}
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
            Screen::Downloads | Screen::Settings | Screen::Provider => self.draw_main_layout(frame),
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

        let provider_names: Vec<&str> = dm_providers::cloud::cloud_providers()
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
                    details_tab: self.details_tab,
                    files_scroll: self.files_scroll,
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
            Screen::Provider => self.draw_provider(frame, main_area),
            Screen::Splash => unreachable!("splash is drawn without the main layout"),
        }
        frame.render_widget(
            Footer {
                message: self.cloud.status().or(self.downloads.status()),
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
        if let Some(entry) = self
            .confirm_delete
            .and_then(|i| self.downloads.entries().get(i))
        {
            frame.render_widget(
                ConfirmPopup {
                    title: "Delete download",
                    message: &format!(
                        "Remove \"{}\" from the list and delete its file from disk?",
                        entry.name
                    ),
                },
                frame.area(),
            );
        }
        if let Some(picker) = &self.cloud.picker {
            let rows: Vec<PickerRow> = picker
                .items
                .iter()
                .zip(&picker.checked)
                .map(|(item, checked)| PickerRow {
                    name: item.name.clone(),
                    size: item.size.map(dm_ui::screens::downloads::format_bytes),
                    checked: *checked,
                })
                .collect();
            frame.render_widget(
                FilePicker {
                    title: &picker.title,
                    rows: &rows,
                    selected: picker.selected,
                    loading: picker.loading,
                    error: picker.error.as_deref(),
                },
                frame.area(),
            );
        }
        if let Some(input) = &self.input_popup {
            frame.render_widget(
                InputPopup {
                    title: &input.title,
                    value: &input.value,
                    masked: matches!(input.target, InputTarget::Field(_, field) if field.is_secret()),
                },
                frame.area(),
            );
        }
    }

    fn draw_provider(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let SidebarItem::Provider(index) = self.sidebar_selection else {
            return;
        };
        let (Some(provider), Some(state)) = (
            dm_providers::cloud::cloud_providers().get(index),
            self.cloud.state(index),
        ) else {
            return;
        };
        let account = &state.account;
        let masked = |_: &str| "•".repeat(8);
        let rows: Vec<FileRow> = state
            .files
            .iter()
            .map(|file| FileRow {
                name: file.name.clone(),
                is_folder: file.is_folder,
                size: file.size.map(dm_ui::screens::downloads::format_bytes),
                checked: state.selected.contains(&file.id),
            })
            .collect();
        let mut items = vec![
            MenuItem {
                label: "Browse files".to_owned(),
                value: if account.is_logged_in() {
                    String::new()
                } else {
                    "login required".to_owned()
                },
            },
            MenuItem {
                label: if account.is_logged_in() {
                    "Logout"
                } else {
                    "Login"
                }
                .to_owned(),
                value: if state.logging_in {
                    "waiting for browser…".to_owned()
                } else if account.is_logged_in() {
                    "logged in".to_owned()
                } else {
                    "not logged in".to_owned()
                },
            },
        ];
        items.extend(provider.fields().iter().map(|spec| {
            let value = spec.field.get(account);
            MenuItem {
                label: spec.label.to_owned(),
                value: if value.is_empty() {
                    "not set".to_owned()
                } else if spec.field.is_secret() {
                    masked(value)
                } else {
                    value.to_owned()
                },
            }
        }));
        let view = if self.browsing {
            let mut path = String::from("/");
            for (_, name) in &state.path {
                path.push_str(name);
                path.push('/');
            }
            ProviderView::Browse {
                path,
                rows: &rows,
                selected: self.selected_file,
                loading: state.loading,
                error: state.error.as_deref(),
            }
        } else {
            ProviderView::Menu {
                items: &items,
                selected: self.selected_menu,
            }
        };
        frame.render_widget(
            ProviderScreen {
                name: provider.name(),
                focused: self.focus == Focus::Content,
                view,
            },
            area,
        );
    }

    fn cloud_file_count(&self) -> usize {
        match self.sidebar_selection {
            SidebarItem::Provider(index) => self.cloud.state(index).map_or(0, |s| s.files.len()),
            _ => 0,
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
