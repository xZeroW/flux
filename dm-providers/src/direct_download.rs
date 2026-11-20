use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::path::Path;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::ProviderCommand;

pub fn commands() -> &'static [ProviderCommand] {
    &[ProviderCommand {
        name: "direct-download",
        description: "Download a file from an HTTP/HTTPS URL",
    }]
}

pub async fn download(url: &str, destination: impl AsRef<Path>) -> Result<()> {
    let destination = destination.as_ref();
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("server returned an error for {url}"))?;

    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    if let Some(parent) = parent {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("creating destination directory {}", parent.display()))?;
    }

    let mut temporary_path = destination.as_os_str().to_os_string();
    temporary_path.push(".part");
    let temporary_path = std::path::PathBuf::from(temporary_path);
    let mut file = File::create(&temporary_path)
        .await
        .with_context(|| format!("creating temporary file {}", temporary_path.display()))?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("receiving data from {url}"))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("writing {}", temporary_path.display()))?;
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
