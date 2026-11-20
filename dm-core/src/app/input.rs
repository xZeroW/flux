use super::{App, Focus, ProviderInput};
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

    pub(super) fn filtered_commands(&self) -> Vec<ProviderCommand> {
        dm_providers::commands_matching(&self.command_query)
    }

    fn activate_sidebar_selection(&mut self) {
        match self.sidebar_selection {
            SidebarItem::Downloads => self.current_screen = Screen::Downloads,
            SidebarItem::Settings => self.current_screen = Screen::Settings,
            _ => {}
        }
    }
}
