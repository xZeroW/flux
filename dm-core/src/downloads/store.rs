use anyhow::{Context, Result};
use dm_ui::screens::downloads::DownloadEntry;
use serde::{Deserialize, Serialize};
use std::{fs, io::ErrorKind, path::PathBuf};

#[derive(Deserialize)]
struct QueueFile {
    #[serde(default)]
    downloads: Vec<DownloadEntry>,
}

#[derive(Serialize)]
struct QueueFileRef<'a> {
    downloads: &'a [DownloadEntry],
}

fn queue_path() -> Result<PathBuf> {
    directories::ProjectDirs::from("com", "Flux", "Flux")
        .map(|dirs| dirs.data_dir().join("downloads.toml"))
        .context("could not determine application data directory")
}

pub fn load() -> Result<Vec<DownloadEntry>> {
    match fs::read_to_string(queue_path()?) {
        Ok(contents) => Ok(toml::from_str::<QueueFile>(&contents)
            .context("failed to parse the download queue")?
            .downloads),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).context("failed to read the download queue"),
    }
}

pub fn save(entries: &[DownloadEntry]) -> Result<()> {
    let path = queue_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(&QueueFileRef { downloads: entries })?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)?;
    Ok(())
}
