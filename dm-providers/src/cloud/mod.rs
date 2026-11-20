use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub mod gdrive;
mod oauth;

const KEYRING_SERVICE: &str = "flux";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudFile {
    pub id: String,
    pub name: String,
    pub is_folder: bool,
    pub downloadable: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Account {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: Option<String>,
}

impl Account {
    pub fn is_logged_in(&self) -> bool {
        self.refresh_token.is_some()
    }

    pub fn has_client(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }

    pub fn load(provider_id: &str) -> Result<Self> {
        match entry(provider_id)?.get_password() {
            Ok(json) => serde_json::from_str(&json).context("stored account is corrupted"),
            Err(keyring::Error::NoEntry) => Ok(Self::default()),
            Err(error) => Err(error).context("reading the OS keyring"),
        }
    }

    pub fn save(&self, provider_id: &str) -> Result<()> {
        let json = serde_json::to_string(self)?;
        entry(provider_id)?
            .set_password(&json)
            .context("writing to the OS keyring")
    }
}

fn entry(provider_id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, provider_id).context("opening the OS keyring")
}

pub type OpenUrl = Box<dyn Fn(String) + Send + Sync>;

#[async_trait::async_trait]
pub trait CloudProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    async fn login(&self, account: &Account, open_url: OpenUrl) -> Result<String>;

    async fn list(&self, account: &Account, folder: Option<&str>) -> Result<Vec<CloudFile>>;
}

pub fn cloud_providers() -> &'static [&'static dyn CloudProvider] {
    &[&gdrive::GoogleDrive]
}
