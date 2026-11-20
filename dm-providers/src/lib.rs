use std::{path::PathBuf, sync::Arc};

pub mod direct_download;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCommand {
    pub name: &'static str,
    pub description: &'static str,
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

    async fn download(
        &self,
        request: DownloadRequest,
        progress: ProgressReporter,
    ) -> anyhow::Result<()>;
}

pub fn commands() -> &'static [ProviderCommand] {
    direct_download::commands()
}
