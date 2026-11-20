use dm_providers::{
    DownloadProgress, DownloadProvider, DownloadRequest, ProgressReporter,
    direct_download::DirectDownloadProvider,
};
use dm_ui::screens::downloads::DownloadEntry;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    watch,
};

enum DownloadEvent {
    Progress(usize, u64, Option<u64>),
    Finished(usize, String),
}

pub struct DownloadManager {
    entries: Vec<DownloadEntry>,
    status: String,
    events_tx: UnboundedSender<DownloadEvent>,
    events_rx: UnboundedReceiver<DownloadEvent>,
    last_progress: Vec<(Instant, u64)>,
    controls: Vec<watch::Sender<bool>>,
    active: Vec<bool>,
    last_persist: Instant,
}

impl DownloadManager {
    pub fn new() -> anyhow::Result<Self> {
        let (events_tx, events_rx) = unbounded_channel();
        let entries = load_entries()?;
        let mut manager = Self {
            entries,
            status: String::new(),
            events_tx,
            events_rx,
            last_progress: Vec::new(),
            controls: Vec::new(),
            active: Vec::new(),
            last_persist: Instant::now(),
        };
        for download in &mut manager.entries {
            if download.status == "Downloading" {
                download.status = "Paused".to_owned();
            }
            manager
                .last_progress
                .push((Instant::now(), download.received));
            let (control, _receiver) = watch::channel(true);
            manager.controls.push(control);
            manager.active.push(false);
        }
        manager.persist()?;
        Ok(manager)
    }

    pub fn entries(&self) -> &[DownloadEntry] {
        &self.entries
    }

    pub fn status(&self) -> Option<&str> {
        (!self.status.is_empty()).then_some(&self.status)
    }

    pub async fn recv_event(&mut self) -> Option<()> {
        let first = self.events_rx.recv().await?;
        self.handle_event(first);
        while let Ok(event) = self.events_rx.try_recv() {
            self.handle_event(event);
        }
        Some(())
    }

    fn handle_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::Progress(index, received, total) => {
                if let Some(download) = self.entries.get_mut(index) {
                    download.received = received;
                    download.total = total;
                }
                if let Some((last_time, last_received)) = self.last_progress.get_mut(index) {
                    let now = Instant::now();
                    let elapsed = now.duration_since(*last_time).as_secs_f64();
                    if elapsed >= 1.0 {
                        let bytes_per_second =
                            received.saturating_sub(*last_received) as f64 / elapsed;
                        if let Some(download) = self.entries.get_mut(index) {
                            download.speed = format_speed(bytes_per_second);
                        }
                        *last_time = now;
                        *last_received = received;
                        if self.last_persist.elapsed() >= Duration::from_secs(2) {
                            let _ = self.persist();
                            self.last_persist = now;
                        }
                    }
                }
            }
            DownloadEvent::Finished(index, status) => {
                if let Some(download) = self.entries.get_mut(index) {
                    download.status = status.clone();
                    download.speed = "—".to_owned();
                    self.status = format!("{}: {status}", download.name);
                }
                if let Some(active) = self.active.get_mut(index) {
                    *active = false;
                }
                let _ = self.persist();
            }
        }
    }

    pub fn start_direct_download(&mut self, url: String) {
        let Some(filename) = url
            .split(['?', '#'])
            .next()
            .and_then(|url| url.rsplit('/').next())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
        else {
            self.status = "URL must include a filename".to_owned();
            return;
        };
        if !url.starts_with("http://") && !url.starts_with("https://") {
            self.status = "URL must start with http:// or https://".to_owned();
            return;
        }
        let Some(downloads) = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
        else {
            self.status = "Could not find the Downloads folder".to_owned();
            return;
        };

        let destination = self.unique_destination(&downloads, &filename);
        let index = self.entries.len();
        self.entries.push(DownloadEntry {
            name: destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            provider: "Direct".to_owned(),
            received: 0,
            total: None,
            speed: "—".to_owned(),
            status: "Downloading".to_owned(),
            url: url.clone(),
            destination: destination.display().to_string(),
        });
        self.last_progress.push((Instant::now(), 0));
        let (control, receiver) = watch::channel(false);
        self.controls.push(control);
        self.active.push(true);
        self.status = format!("Downloading to {}", destination.display());
        self.spawn_download(index, receiver);
        let _ = self.persist();
    }

    pub fn toggle_pause(&mut self, index: usize) {
        let Some(download) = self.entries.get_mut(index) else {
            return;
        };
        if download.status == "Downloading" {
            download.status = "Paused".to_owned();
            download.speed = "—".to_owned();
            if let Some(control) = self.controls.get(index) {
                let _ = control.send(true);
            }
        } else if download.status == "Paused" || download.status.starts_with("Failed:") {
            download.status = "Downloading".to_owned();
            if let Some(progress) = self.last_progress.get_mut(index) {
                *progress = (Instant::now(), download.received);
            }
            if !self.active.get(index).copied().unwrap_or(false) {
                let (control, receiver) = watch::channel(false);
                self.controls[index] = control;
                self.active[index] = true;
                self.spawn_download(index, receiver);
            } else if let Some(control) = self.controls.get(index) {
                let _ = control.send(false);
            }
        }
        let _ = self.persist();
    }

    fn spawn_download(&self, index: usize, paused: watch::Receiver<bool>) {
        let entry = &self.entries[index];
        let url = entry.url.clone();
        let destination = PathBuf::from(&entry.destination);
        let tx = self.events_tx.clone();
        tokio::spawn(async move {
            let progress_tx = tx.clone();
            let progress: ProgressReporter =
                std::sync::Arc::new(move |progress: DownloadProgress| {
                    let _ = progress_tx.send(DownloadEvent::Progress(
                        index,
                        progress.received_bytes,
                        progress.total_bytes,
                    ));
                });
            let provider = DirectDownloadProvider;
            let result = provider
                .download(
                    DownloadRequest {
                        url,
                        destination,
                        paused,
                    },
                    progress,
                )
                .await;
            let status = match result {
                Ok(()) => "Complete".to_owned(),
                Err(error) => format!("Failed: {error:#}"),
            };
            let _ = tx.send(DownloadEvent::Finished(index, status));
        });
    }

    fn unique_destination(&self, directory: &Path, filename: &str) -> PathBuf {
        let original = directory.join(filename);
        let is_in_use = |candidate: &Path| {
            self.entries
                .iter()
                .any(|download| Path::new(&download.destination) == candidate)
                || candidate.exists()
        };
        if !is_in_use(&original) {
            return original;
        }

        let path = Path::new(filename);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = path.extension().map(|ext| ext.to_string_lossy());
        for suffix in 1.. {
            let name = match &extension {
                Some(extension) => format!("{stem} ({suffix}).{extension}"),
                None => format!("{stem} ({suffix})"),
            };
            let candidate = directory.join(name);
            if !is_in_use(&candidate) {
                return candidate;
            }
        }
        unreachable!("an unused download filename should always be available")
    }

    fn persist(&self) -> anyhow::Result<()> {
        let path = queue_path()?;
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("download queue path has no parent"))?;
        fs::create_dir_all(parent)?;
        let contents = toml::to_string_pretty(&QueueFile {
            downloads: self.entries.clone(),
        })?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, contents)?;
        fs::rename(temporary, path)?;
        Ok(())
    }
}

fn format_speed(bytes_per_second: f64) -> String {
    const KIB: f64 = 1024.0;
    if bytes_per_second >= KIB * KIB {
        format!("{:.1} MiB/s", bytes_per_second / (KIB * KIB))
    } else if bytes_per_second >= KIB {
        format!("{:.1} KiB/s", bytes_per_second / KIB)
    } else {
        format!("{:.0} B/s", bytes_per_second)
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new().expect("failed to load the download queue")
    }
}

impl Drop for DownloadManager {
    fn drop(&mut self) {
        let _ = self.persist();
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct QueueFile {
    #[serde(default)]
    downloads: Vec<DownloadEntry>,
}

fn queue_path() -> anyhow::Result<PathBuf> {
    directories::ProjectDirs::from("com", "Flux", "Flux")
        .map(|dirs| dirs.data_dir().join("downloads.toml"))
        .ok_or_else(|| anyhow::anyhow!("could not determine application data directory"))
}

fn load_entries() -> anyhow::Result<Vec<DownloadEntry>> {
    match fs::read_to_string(queue_path()?) {
        Ok(contents) => Ok(toml::from_str::<QueueFile>(&contents)?.downloads),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}
