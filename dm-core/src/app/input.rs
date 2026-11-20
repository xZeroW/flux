use super::{App, Focus, InputTarget, ProviderInput};
use crate::settings::Settings;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use dm_providers::ProviderCommand;
use dm_ui::{screens::Screen, widgets::sidebar::SidebarItem};

impl App {
    pub(super) fn handle_event(&mut self, event: Option<Event>) -> Result<()> {
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
        if self.confirm_delete.is_some() {
            self.handle_confirm_delete_key(key_event.code);
            return Ok(());
        }
        if self.input_popup.is_some() {
            self.handle_input_key(key_event.code);
            return Ok(());
        }
        if self.command_palette_open {
            self.handle_command_palette_key(key_event.code);
            return Ok(());
        }

        if self.focus == Focus::Content
            && matches!(self.current_screen, Screen::Provider)
            && self.handle_provider_key(key_event.code)
        {
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
            KeyCode::Char('d')
                if matches!(self.current_screen, Screen::Downloads)
                    && self.focus == Focus::Content =>
            {
                self.remove_selected_download(false);
            }
            KeyCode::Char('D')
                if matches!(self.current_screen, Screen::Downloads)
                    && self.focus == Focus::Content
                    && self.selected_download < self.downloads.entries().len() =>
            {
                self.confirm_delete = Some(self.selected_download);
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
                    .step(true, dm_providers::cloud::cloud_providers().len());
                self.show_sidebar_selection();
            }
            KeyCode::Char('k') | KeyCode::Up if self.focus == Focus::Sidebar => {
                self.sidebar_selection = self
                    .sidebar_selection
                    .step(false, dm_providers::cloud::cloud_providers().len());
                self.show_sidebar_selection();
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

    fn handle_confirm_delete_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                if let Some(index) = self.confirm_delete.take() {
                    self.selected_download = index;
                    self.remove_selected_download(true);
                }
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.confirm_delete = None,
            _ => {}
        }
    }

    fn remove_selected_download(&mut self, delete_file: bool) {
        self.downloads.remove(self.selected_download, delete_file);
        self.selected_download = self
            .selected_download
            .min(self.downloads.entries().len().saturating_sub(1));
    }

    fn handle_command_palette_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => self.command_palette_open = false,
            KeyCode::Enter => {
                let commands = self.filtered_commands();
                self.command_palette_open = false;
                if let Some(command) = commands.get(self.selected_command) {
                    self.input_popup = Some(ProviderInput {
                        target: InputTarget::Download(command.provider_id),
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
                    match input.target {
                        InputTarget::Download(provider_id) => {
                            self.downloads.start_download(provider_id, input.value);
                            self.selected_download =
                                self.downloads.entries().len().saturating_sub(1);
                        }
                        InputTarget::ClientId(index) => {
                            self.cloud.set_client_id(index, input.value)
                        }
                        InputTarget::ClientSecret(index) => {
                            self.cloud.set_client_secret(index, input.value)
                        }
                    }
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

    pub(super) fn filtered_commands(&self) -> Vec<ProviderCommand> {
        dm_providers::commands_matching(&self.command_query)
    }

    fn show_sidebar_selection(&mut self) {
        match self.sidebar_selection {
            SidebarItem::Downloads => self.current_screen = Screen::Downloads,
            SidebarItem::Settings => self.current_screen = Screen::Settings,
            SidebarItem::Provider(_) => {
                self.current_screen = Screen::Provider;
                self.browsing = false;
                self.selected_menu = 0;
            }
        }
    }

    fn activate_sidebar_selection(&mut self) {
        self.show_sidebar_selection();
        if matches!(self.sidebar_selection, SidebarItem::Provider(_)) {
            self.focus = Focus::Content;
        }
    }

    fn selected_provider(&self) -> Option<usize> {
        match self.sidebar_selection {
            SidebarItem::Provider(index) => Some(index),
            _ => None,
        }
    }

    fn handle_provider_key(&mut self, code: KeyCode) -> bool {
        let Some(index) = self.selected_provider() else {
            return false;
        };
        if self.browsing {
            let count = self.cloud.state(index).map_or(0, |state| state.files.len());
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.selected_file = (self.selected_file + 1).min(count.saturating_sub(1));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.selected_file = self.selected_file.saturating_sub(1);
                }
                KeyCode::Enter => {
                    self.cloud.open_folder(index, self.selected_file);
                    self.selected_file = 0;
                }
                KeyCode::Char(' ') => self.cloud.toggle_selected(index, self.selected_file),
                KeyCode::Char('g') => self.download_selected_cloud_files(index),
                KeyCode::Backspace => {
                    if self.cloud.go_up(index) {
                        self.selected_file = 0;
                    }
                }
                KeyCode::Esc => {
                    if self.cloud.go_up(index) {
                        self.selected_file = 0;
                    } else {
                        self.browsing = false;
                    }
                }
                _ => return false,
            }
            return true;
        }
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected_menu = (self.selected_menu + 1).min(3)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_menu = self.selected_menu.saturating_sub(1)
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_provider_menu(index),
            KeyCode::Esc => {
                self.current_screen = Screen::Downloads;
                self.sidebar_selection = SidebarItem::Downloads;
            }
            _ => return false,
        }
        true
    }

    fn download_selected_cloud_files(&mut self, index: usize) {
        let Some(provider) = dm_providers::cloud::cloud_providers().get(index) else {
            return;
        };
        let files = self.cloud.take_selected(index);
        for file in &files {
            let input = dm_providers::cloud::gdrive::download_input(file);
            self.downloads.start_download(provider.id(), input);
        }
        if !files.is_empty() {
            self.selected_download = self.downloads.entries().len().saturating_sub(1);
        }
    }

    fn activate_provider_menu(&mut self, index: usize) {
        let Some(state) = self.cloud.state(index) else {
            return;
        };
        let logged_in = state.account.is_logged_in();
        match self.selected_menu {
            0 if logged_in => {
                self.browsing = true;
                self.selected_file = 0;
                self.cloud.refresh(index);
            }
            1 if logged_in => self.cloud.logout(index),
            1 => self.cloud.login(index),
            2 => {
                self.input_popup = Some(ProviderInput {
                    target: InputTarget::ClientId(index),
                    title: "OAuth client ID".to_owned(),
                    value: String::new(),
                });
            }
            3 => {
                self.input_popup = Some(ProviderInput {
                    target: InputTarget::ClientSecret(index),
                    title: "OAuth client secret".to_owned(),
                    value: String::new(),
                });
            }
            _ => {}
        }
    }
}
