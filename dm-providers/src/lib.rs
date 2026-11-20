use std::{path::PathBuf, sync::Arc};

pub mod direct_download;
pub mod torrent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub provider_id: &'static str,
    pub input_prompt: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationKind {
    File,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDownload {
    pub name: String,
    pub destination: DestinationKind,
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub destination: PathBuf,
    pub paused: tokio::sync::watch::Receiver<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub type ProgressReporter = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[async_trait::async_trait]
pub trait DownloadProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn commands(&self) -> &'static [ProviderCommand];

    fn resolve(&self, input: &str) -> Result<ResolvedDownload, String>;

    async fn download(
        &self,
        request: DownloadRequest,
        progress: ProgressReporter,
    ) -> anyhow::Result<()>;
}

pub fn providers() -> &'static [&'static dyn DownloadProvider] {
    &[
        &direct_download::DirectDownloadProvider,
        &torrent::TorrentProvider,
    ]
}

pub fn commands_matching(query: &str) -> Vec<ProviderCommand> {
    let query = query.to_lowercase();
    providers()
        .iter()
        .flat_map(|provider| provider.commands().iter().copied())
        .filter(|command| command.name.to_lowercase().contains(&query))
        .collect()
}

pub fn find_provider(label: &str) -> Option<&'static dyn DownloadProvider> {
    providers()
        .iter()
        .copied()
        .find(|provider| provider.name() == label || provider.id().eq_ignore_ascii_case(label))
}
