use dm_providers::cloud::{Account, CloudFile, CloudProvider, cloud_providers};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

enum CloudEvent {
    Status(String),
    LoggedIn {
        index: usize,
        result: Result<String, String>,
    },
    Listed {
        index: usize,
        generation: u64,
        result: Result<Vec<CloudFile>, String>,
    },
}

#[derive(Default)]
pub struct ProviderState {
    pub account: Account,
    pub logging_in: bool,
    pub path: Vec<(String, String)>,
    pub files: Vec<CloudFile>,
    pub loading: bool,
    pub error: Option<String>,
    pub selected: HashSet<String>,
    generation: u64,
}

const STATUS_TTL: Duration = Duration::from_secs(5);

pub struct CloudManager {
    states: Vec<ProviderState>,
    status: String,
    status_set_at: Instant,
    tx: UnboundedSender<CloudEvent>,
    rx: UnboundedReceiver<CloudEvent>,
}

impl CloudManager {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        let mut status = String::new();
        let states = cloud_providers()
            .iter()
            .map(|provider| {
                let account = Account::load(provider.id()).unwrap_or_else(|error| {
                    status = format!("{}: {error:#}", provider.name());
                    Account::default()
                });
                ProviderState {
                    account,
                    ..ProviderState::default()
                }
            })
            .collect();
        Self {
            states,
            status,
            status_set_at: Instant::now(),
            tx,
            rx,
        }
    }

    pub fn status(&self) -> Option<&str> {
        let waiting = self.states.iter().any(|state| state.logging_in);
        (!self.status.is_empty() && (waiting || self.status_set_at.elapsed() < STATUS_TTL))
            .then_some(&self.status)
    }

    fn set_status(&mut self, message: String) {
        self.status = message;
        self.status_set_at = Instant::now();
    }

    pub fn state(&self, index: usize) -> Option<&ProviderState> {
        self.states.get(index)
    }

    fn provider(index: usize) -> Option<&'static dyn CloudProvider> {
        cloud_providers().get(index).copied()
    }

    pub async fn recv_event(&mut self) -> Option<()> {
        let event = self.rx.recv().await?;
        self.handle_event(event);
        Some(())
    }

    fn handle_event(&mut self, event: CloudEvent) {
        match event {
            CloudEvent::Status(message) => self.set_status(message),
            CloudEvent::LoggedIn { index, result } => {
                let Some(state) = self.states.get_mut(index) else {
                    return;
                };
                state.logging_in = false;
                match result {
                    Ok(token) => {
                        state.account.refresh_token = Some(token);
                        let message = match self.save(index) {
                            Ok(()) => "Logged in".to_owned(),
                            Err(error) => format!("Logged in, but could not save: {error}"),
                        };
                        self.set_status(message);
                    }
                    Err(error) => self.set_status(format!("Login failed: {error}")),
                }
            }
            CloudEvent::Listed {
                index,
                generation,
                result,
            } => {
                let Some(state) = self.states.get_mut(index) else {
                    return;
                };
                if state.generation != generation {
                    return; // the user already navigated elsewhere
                }
                state.loading = false;
                match result {
                    Ok(files) => {
                        state.files = files;
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
    }

    fn save(&mut self, index: usize) -> Result<(), String> {
        let (Some(provider), Some(state)) = (Self::provider(index), self.states.get(index)) else {
            return Ok(());
        };
        state
            .account
            .save(provider.id())
            .map_err(|error| format!("{error:#}"))
    }

    fn update_account(&mut self, index: usize, change: impl FnOnce(&mut Account)) {
        let Some(state) = self.states.get_mut(index) else {
            return;
        };
        change(&mut state.account);
        if let Err(error) = self.save(index) {
            self.set_status(format!("Could not save to the keyring: {error}"));
        }
    }

    pub fn set_client_id(&mut self, index: usize, value: String) {
        self.update_account(index, |account| account.client_id = value.trim().to_owned());
    }

    pub fn set_client_secret(&mut self, index: usize, value: String) {
        self.update_account(index, |account| {
            account.client_secret = value.trim().to_owned()
        });
    }

    pub fn logout(&mut self, index: usize) {
        self.update_account(index, |account| account.refresh_token = None);
        if let Some(state) = self.states.get_mut(index) {
            state.files.clear();
            state.path.clear();
            state.selected.clear();
            state.generation += 1;
        }
        self.set_status("Logged out".to_owned());
    }

    pub fn login(&mut self, index: usize) {
        let (Some(provider), Some(state)) = (Self::provider(index), self.states.get_mut(index))
        else {
            return;
        };
        if state.logging_in {
            return;
        }
        if !state.account.has_client() {
            self.set_status("Set the client ID and client secret first".to_owned());
            return;
        }
        state.logging_in = true;
        let account = state.account.clone();
        self.set_status("Opening your browser to sign in…".to_owned());
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let url_tx = tx.clone();
            let open_url: dm_providers::cloud::OpenUrl = Box::new(move |url| {
                let opened = open::that_detached(&url).is_ok();
                let _ = url_tx.send(CloudEvent::Status(if opened {
                    "Waiting for you to sign in in the browser…".to_owned()
                } else {
                    format!("Open this URL to sign in: {url}")
                }));
            });
            let result = provider
                .login(&account, open_url)
                .await
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(CloudEvent::LoggedIn { index, result });
        });
    }

    pub fn refresh(&mut self, index: usize) {
        let (Some(provider), Some(state)) = (Self::provider(index), self.states.get_mut(index))
        else {
            return;
        };
        if !state.account.is_logged_in() {
            return;
        }
        state.generation += 1;
        state.loading = true;
        state.error = None;
        state.files.clear();
        let generation = state.generation;
        let account = state.account.clone();
        let folder = state.path.last().map(|(id, _)| id.clone());
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = provider
                .list(&account, folder.as_deref())
                .await
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(CloudEvent::Listed {
                index,
                generation,
                result,
            });
        });
    }

    pub fn open_folder(&mut self, index: usize, file_index: usize) {
        let Some(state) = self.states.get_mut(index) else {
            return;
        };
        let Some(file) = state.files.get(file_index).filter(|file| file.is_folder) else {
            return;
        };
        state.path.push((file.id.clone(), file.name.clone()));
        self.refresh(index);
    }

    pub fn go_up(&mut self, index: usize) -> bool {
        let Some(state) = self.states.get_mut(index) else {
            return false;
        };
        if state.path.pop().is_none() {
            return false;
        }
        self.refresh(index);
        true
    }

    pub fn take_selected(&mut self, index: usize) -> Vec<CloudFile> {
        let Some(state) = self.states.get_mut(index) else {
            return Vec::new();
        };
        let selected = std::mem::take(&mut state.selected);
        state
            .files
            .iter()
            .filter(|file| selected.contains(&file.id))
            .cloned()
            .collect()
    }

    pub fn toggle_selected(&mut self, index: usize, file_index: usize) {
        let Some(state) = self.states.get_mut(index) else {
            return;
        };
        let Some(file) = state
            .files
            .get(file_index)
            .filter(|file| !file.is_folder && file.downloadable)
        else {
            return;
        };
        if !state.selected.remove(&file.id) {
            state.selected.insert(file.id.clone());
        }
    }
}
