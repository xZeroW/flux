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
    /// Opaque "you are logged in" credential returned by the provider's login.
    pub refresh_token: Option<String>,
}

/// An account setting the user can edit from the provider screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountField {
    ClientId,
    ClientSecret,
}

impl AccountField {
    pub fn is_secret(self) -> bool {
        matches!(self, Self::ClientSecret)
    }

    pub fn get(self, account: &Account) -> &str {
        match self {
            Self::ClientId => &account.client_id,
            Self::ClientSecret => &account.client_secret,
        }
    }

    pub fn set(self, account: &mut Account, value: String) {
        let value = value.trim().to_owned();
        match self {
            Self::ClientId => account.client_id = value,
            Self::ClientSecret => account.client_secret = value,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub field: AccountField,
    pub label: &'static str,
}

impl Account {
    pub fn is_logged_in(&self) -> bool {
        self.refresh_token.is_some()
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

    /// The settings this provider needs, in display order.
    fn fields(&self) -> &'static [FieldSpec];

    /// Whether all required settings are filled in so login can start.
    fn is_configured(&self, account: &Account) -> bool;

    /// The download input (see [`crate::DownloadProvider::resolve`]) for a file.
    fn download_input(&self, file: &CloudFile) -> String;

    async fn login(&self, account: &Account, open_url: OpenUrl) -> Result<String>;

    async fn list(&self, account: &Account, folder: Option<&str>) -> Result<Vec<CloudFile>>;
}

pub fn cloud_providers() -> &'static [&'static dyn CloudProvider] {
    &[&gdrive::GoogleDrive]
}
