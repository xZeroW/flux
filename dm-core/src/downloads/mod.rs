mod store;

use dm_providers::{DestinationKind, DownloadProgress, DownloadRequest, ProgressReporter};
use dm_ui::screens::downloads::{DownloadEntry, DownloadStatus};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    watch,
};

const PERSIST_INTERVAL: Duration = Duration::from_secs(2);
const SPEED_INTERVAL: Duration = Duration::from_secs(1);
const NO_SPEED: &str = "—";

enum DownloadEvent {
    Progress {
        index: usize,
        received: u64,
        total: Option<u64>,
    },
    Finished {
        index: usize,
        status: DownloadStatus,
    },
}

#[derive(Default)]
struct Task {
    control: Option<watch::Sender<bool>>,
    speed_sample: Option<(Instant, u64)>,
}

pub struct DownloadManager {
    entries: Vec<DownloadEntry>,
    tasks: Vec<Task>,
    status: String,
    events_tx: UnboundedSender<DownloadEvent>,
    events_rx: UnboundedReceiver<DownloadEvent>,
    last_persist: Instant,
}

impl DownloadManager {
    pub fn new() -> anyhow::Result<Self> {
        let (events_tx, events_rx) = unbounded_channel();
        let mut entries = store::load()?;
        for entry in &mut entries {
            if entry.status == DownloadStatus::Downloading {
                entry.status = DownloadStatus::Paused;
            }
        }
        let tasks = entries.iter().map(|_| Task::default()).collect();
        let manager = Self {
            entries,
            tasks,
            status: String::new(),
            events_tx,
            events_rx,
            last_persist: Instant::now(),
        };
        store::save(&manager.entries)?;
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
            DownloadEvent::Progress {
                index,
                received,
                total,
            } => self.record_progress(index, received, total),
            DownloadEvent::Finished { index, status } => {
                let Some(entry) = self.entries.get_mut(index) else {
                    return;
                };
                entry.status = status.clone();
                entry.speed = NO_SPEED.to_owned();
                self.status = format!("{}: {status}", entry.name);
                self.tasks[index] = Task::default();
                self.save();
            }
        }
    }

    fn record_progress(&mut self, index: usize, received: u64, total: Option<u64>) {
        let (Some(entry), Some(task)) = (self.entries.get_mut(index), self.tasks.get_mut(index))
        else {
            return;
        };
        entry.received = received;
        entry.total = total;

        let now = Instant::now();
        let (sampled_at, sampled_bytes) = *task.speed_sample.get_or_insert((now, received));
        let elapsed = now.duration_since(sampled_at);
        if elapsed < SPEED_INTERVAL {
            return;
        }
        let bytes = received.saturating_sub(sampled_bytes);
        entry.speed = format_speed(bytes as f64 / elapsed.as_secs_f64());
        task.speed_sample = Some((now, received));
        if self.last_persist.elapsed() >= PERSIST_INTERVAL {
            self.save();
        }
    }

    pub fn start_download(&mut self, provider_id: &str, input: String) {
        let Some(provider) = dm_providers::find_provider(provider_id) else {
            self.status = format!("Unknown provider: {provider_id}");
            return;
        };
        let input = input.trim().to_owned();
        let resolved = match provider.resolve(&input) {
            Ok(resolved) => resolved,
            Err(message) => {
                self.status = message;
                return;
            }
        };
        let Some(downloads) = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
        else {
            self.status = "Could not find the Downloads folder".to_owned();
            return;
        };

        let (name, destination) = match resolved.destination {
            DestinationKind::File => {
                let destination = self.unique_destination(&downloads, &resolved.name);
                let name = destination
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                (name, destination)
            }
            DestinationKind::Folder => (resolved.name, downloads),
        };
        self.status = format!("Downloading to {}", destination.display());
        self.entries.push(DownloadEntry {
            name,
            provider: provider.name().to_owned(),
            received: 0,
            total: None,
            speed: NO_SPEED.to_owned(),
            status: DownloadStatus::Downloading,
            url: input,
            destination: destination.display().to_string(),
        });
        self.tasks.push(Task::default());
        self.spawn_download(self.entries.len() - 1);
        self.save();
    }

    pub fn toggle_pause(&mut self, index: usize) {
        let (Some(entry), Some(task)) = (self.entries.get_mut(index), self.tasks.get_mut(index))
        else {
            return;
        };
        match entry.status {
            DownloadStatus::Downloading => {
                entry.status = DownloadStatus::Paused;
                entry.speed = NO_SPEED.to_owned();
                if let Some(control) = &task.control {
                    let _ = control.send(true);
                }
            }
            DownloadStatus::Paused | DownloadStatus::Failed(_) => {
                entry.status = DownloadStatus::Downloading;
                task.speed_sample = Some((Instant::now(), entry.received));
                match &task.control {
                    Some(control) => {
                        let _ = control.send(false);
                    }
                    None => self.spawn_download(index),
                }
            }
            DownloadStatus::Complete => return,
        }
        self.save();
    }

    fn spawn_download(&mut self, index: usize) {
        let (control, paused) = watch::channel(false);
        self.tasks[index].control = Some(control);

        let entry = &self.entries[index];
        let provider = dm_providers::find_provider(&entry.provider);
        let request = DownloadRequest {
            url: entry.url.clone(),
            destination: PathBuf::from(&entry.destination),
            paused,
        };
        let tx = self.events_tx.clone();
        tokio::spawn(async move {
            let progress_tx = tx.clone();
            let progress: ProgressReporter = Arc::new(move |progress: DownloadProgress| {
                let _ = progress_tx.send(DownloadEvent::Progress {
                    index,
                    received: progress.received_bytes,
                    total: progress.total_bytes,
                });
            });
            let result = match provider {
                Some(provider) => provider.download(request, progress).await,
                None => Err(anyhow::anyhow!("provider is no longer available")),
            };
            let status = match result {
                Ok(()) => DownloadStatus::Complete,
                Err(error) => DownloadStatus::Failed(format!("{error:#}")),
            };
            let _ = tx.send(DownloadEvent::Finished { index, status });
        });
    }

    fn unique_destination(&self, directory: &Path, filename: &str) -> PathBuf {
        let is_free = |candidate: &Path| {
            !candidate.exists()
                && !self
                    .entries
                    .iter()
                    .any(|entry| Path::new(&entry.destination) == candidate)
        };
        let original = directory.join(filename);
        if is_free(&original) {
            return original;
        }

        let path = Path::new(filename);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = path.extension().map(|ext| ext.to_string_lossy());
        (1..)
            .map(|suffix| match &extension {
                Some(extension) => directory.join(format!("{stem} ({suffix}).{extension}")),
                None => directory.join(format!("{stem} ({suffix})")),
            })
            .find(|candidate| is_free(candidate))
            .expect("an unused download filename should always be available")
    }

    fn save(&mut self) {
        self.last_persist = Instant::now();
        if let Err(error) = store::save(&self.entries) {
            self.status = format!("Could not save the download queue: {error:#}");
        }
    }
}

impl Drop for DownloadManager {
    fn drop(&mut self) {
        let _ = store::save(&self.entries);
    }
}

fn format_speed(bytes_per_second: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * KIB;
    if bytes_per_second >= MIB {
        format!("{:.1} MiB/s", bytes_per_second / MIB)
    } else if bytes_per_second >= KIB {
        format!("{:.1} KiB/s", bytes_per_second / KIB)
    } else {
        format!("{bytes_per_second:.0} B/s")
    }
}
