use anyhow::{Context, Result, bail};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Magnet, Session};
use std::{sync::Arc, time::Duration};
use tokio::sync::OnceCell;

use crate::{
    DestinationKind, DownloadProgress, DownloadProvider, DownloadRequest, ProgressReporter,
    ProviderCommand, ResolvedDownload,
};

pub fn commands() -> &'static [ProviderCommand] {
    &[ProviderCommand {
        name: "torrent",
        description: "Download a torrent from a magnet link or .torrent URL",
        provider_id: "torrent",
        input_prompt: "Magnet link or .torrent URL",
    }]
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TorrentProvider;

static SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

async fn session(output_folder: &std::path::Path) -> Result<Arc<Session>> {
    SESSION
        .get_or_try_init(|| async {
            Session::new(output_folder.to_path_buf())
                .await
                .context("starting the torrent session")
        })
        .await
        .cloned()
}

#[async_trait::async_trait]
impl DownloadProvider for TorrentProvider {
    fn id(&self) -> &'static str {
        "torrent"
    }

    fn name(&self) -> &'static str {
        "Torrent"
    }

    fn commands(&self) -> &'static [ProviderCommand] {
        commands()
    }

    fn resolve(&self, input: &str) -> Result<ResolvedDownload, String> {
        let name = if input.starts_with("magnet:") {
            let magnet =
                Magnet::parse(input).map_err(|error| format!("Invalid magnet link: {error}"))?;
            magnet.name.unwrap_or_else(|| "magnet download".to_owned())
        } else if input.starts_with("http://") || input.starts_with("https://") {
            input
                .split(['?', '#'])
                .next()
                .and_then(|url| url.rsplit('/').next())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "URL must include a filename".to_owned())?
                .to_owned()
        } else {
            return Err("Enter a magnet link or a http(s) URL to a .torrent file".to_owned());
        };
        Ok(ResolvedDownload {
            name,
            destination: DestinationKind::Folder,
        })
    }

    async fn download(&self, request: DownloadRequest, progress: ProgressReporter) -> Result<()> {
        let DownloadRequest {
            url,
            destination,
            mut paused,
        } = request;
        let start_paused = *paused.borrow();
        let session = session(&destination).await?;
        let response = session
            .add_torrent(
                AddTorrent::from_url(url.clone()),
                Some(AddTorrentOptions {
                    paused: start_paused,
                    overwrite: true,
                    output_folder: Some(destination.display().to_string()),
                    ..Default::default()
                }),
            )
            .await
            .with_context(|| format!("adding torrent {url}"))?;
        let handle = match response {
            AddTorrentResponse::Added(_, handle)
            | AddTorrentResponse::AlreadyManaged(_, handle) => handle,
            AddTorrentResponse::ListOnly(_) => bail!("torrent was only listed, not added"),
        };
        if !start_paused && handle.is_paused() {
            session.unpause(&handle).await?;
        }

        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        loop {
            tokio::select! {
                changed = paused.changed() => {
                    changed.context("download pause channel closed")?;
                    let want_paused = *paused.borrow_and_update();
                    if want_paused && !handle.is_paused() {
                        session.pause(&handle).await?;
                    } else if !want_paused && handle.is_paused() {
                        session.unpause(&handle).await?;
                    }
                }
                _ = ticker.tick() => {
                    let stats = handle.stats();
                    if let Some(error) = stats.error {
                        bail!(error);
                    }
                    progress(DownloadProgress {
                        received_bytes: stats.progress_bytes,
                        total_bytes: (stats.total_bytes > 0).then_some(stats.total_bytes),
                    });
                    if stats.finished {
                        break;
                    }
                }
            }
        }

        session.delete(handle.id().into(), false).await?;
        Ok(())
    }
}
