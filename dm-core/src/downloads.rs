use dm_providers::{
    DownloadProgress, DownloadProvider, DownloadRequest, ProgressReporter,
    direct_download::DirectDownloadProvider,
};
use dm_ui::screens::downloads::DownloadEntry;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

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
}

impl DownloadManager {
    pub fn new() -> Self {
        let (events_tx, events_rx) = unbounded_channel();
        Self {
            entries: Vec::new(),
            status: String::new(),
            events_tx,
            events_rx,
            last_progress: Vec::new(),
        }
    }

    pub fn entries(&self) -> &[DownloadEntry] {
        &self.entries
    }

    pub fn status(&self) -> Option<&str> {
        (!self.status.is_empty()).then_some(&self.status)
    }

    pub async fn recv_event(&mut self) -> Option<()> {
        self.events_rx
            .recv()
            .await
            .map(|event| self.handle_event(event))
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
                    }
                }
            }
            DownloadEvent::Finished(index, status) => {
                if let Some(download) = self.entries.get_mut(index) {
                    download.status = status.clone();
                    download.speed = "—".to_owned();
                    self.status = format!("{}: {status}", download.name);
                }
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
        let tx = self.events_tx.clone();
        self.status = format!("Downloading to {}", destination.display());
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
                .download(DownloadRequest { url, destination }, progress)
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
        Self::new()
    }
}
