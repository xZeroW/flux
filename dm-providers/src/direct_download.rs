use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::path::Path;
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::watch};

use crate::{
    DownloadProgress, DownloadProvider, DownloadRequest, ProgressReporter, ProviderCommand,
};

pub fn commands() -> &'static [ProviderCommand] {
    &[ProviderCommand {
        name: "direct-download",
        description: "Download a file from an HTTP/HTTPS URL",
    }]
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DirectDownloadProvider;

#[async_trait::async_trait]
impl DownloadProvider for DirectDownloadProvider {
    fn id(&self) -> &'static str {
        "direct"
    }

    fn name(&self) -> &'static str {
        "Direct download"
    }

    fn commands(&self) -> &'static [ProviderCommand] {
        commands()
    }

    async fn download(&self, request: DownloadRequest, progress: ProgressReporter) -> Result<()> {
        download_with_progress(
            &request.url,
            &request.destination,
            request.paused,
            move |received, total| {
                progress(DownloadProgress {
                    received_bytes: received,
                    total_bytes: total,
                });
            },
        )
        .await
    }
}

pub async fn download(url: &str, destination: impl AsRef<Path>) -> Result<()> {
    let (_pause_tx, pause_rx) = watch::channel(false);
    download_with_progress(url, destination, pause_rx, |_, _| {}).await
}

pub async fn download_with_progress(
    url: &str,
    destination: impl AsRef<Path>,
    mut paused: watch::Receiver<bool>,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    let destination = destination.as_ref();
    let mut temporary_path = destination.as_os_str().to_os_string();
    temporary_path.push(".part");
    let temporary_path = std::path::PathBuf::from(temporary_path);
    let offset = tokio::fs::metadata(&temporary_path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("server returned an error for {url}"))?;
    let append = offset > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let offset = if append { offset } else { 0 };

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    if let Some(parent) = parent {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("creating destination directory {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&temporary_path)
        .await
        .with_context(|| format!("creating temporary file {}", temporary_path.display()))?;
    let total = response.content_length().map(|length| length + offset);
    let mut received = offset;
    let mut stream = response.bytes_stream();

    loop {
        while *paused.borrow() {
            paused
                .changed()
                .await
                .context("download pause channel closed")?;
        }
        let next_chunk = tokio::select! {
            biased;
            changed = paused.changed() => {
                changed.context("download pause channel closed")?;
                continue;
            }
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = next_chunk else {
            break;
        };
        let chunk = chunk.with_context(|| format!("receiving data from {url}"))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("writing {}", temporary_path.display()))?;
        received += chunk.len() as u64;
        progress(received, total);
    }
    file.flush()
        .await
        .with_context(|| format!("flushing {}", temporary_path.display()))?;
    drop(file);

    if tokio::fs::try_exists(destination).await? {
        tokio::fs::remove_file(destination)
            .await
            .with_context(|| format!("replacing {}", destination.display()))?;
    }
    tokio::fs::rename(&temporary_path, destination)
        .await
        .with_context(|| format!("moving download to {}", destination.display()))?;

    Ok(())
}
