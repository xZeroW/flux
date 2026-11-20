use std::{path::PathBuf, sync::Arc};

pub mod cloud;
pub mod direct_download;
pub mod mediafire;
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

/// One downloadable file inside a link that points at several (e.g. a folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionItem {
    /// Path shown to the user, relative to the collection root.
    pub name: String,
    pub size: Option<u64>,
    /// A plain download input for this single file (see `resolve`).
    pub input: String,
}

pub type ProgressReporter = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[async_trait::async_trait]
pub trait DownloadProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn commands(&self) -> &'static [ProviderCommand];

    fn resolve(&self, input: &str) -> Result<ResolvedDownload, String>;

    /// True when `input` points at several files that the user should pick from.
    fn is_collection(&self, _input: &str) -> bool {
        false
    }

    /// List the files behind a collection input.
    async fn list_collection(&self, _input: &str) -> anyhow::Result<Vec<CollectionItem>> {
        anyhow::bail!("{} has no file lists", self.name())
    }

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
        &cloud::gdrive::GoogleDrive,
        &mediafire::MediaFire,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_are_found_by_id_and_name() {
        assert_eq!(find_provider("torrent").map(|p| p.id()), Some("torrent"));
        assert_eq!(
            find_provider("Direct download").map(|p| p.id()),
            Some("direct")
        );
        assert!(find_provider("nope").is_none());
    }

    #[test]
    fn providers_validate_their_input() {
        let direct = find_provider("direct").unwrap();
        assert_eq!(
            direct.resolve("https://x.org/a/file.iso?t=1").unwrap().name,
            "file.iso"
        );
        assert!(direct.resolve("ftp://x.org/file").is_err());
        assert!(direct.resolve("https://x.org/").is_err());

        let torrent = find_provider("torrent").unwrap();
        let magnet = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Cool+Stuff";
        assert_eq!(
            torrent.resolve(magnet).unwrap().destination,
            DestinationKind::Folder
        );
        assert!(torrent.resolve("magnet:?foo=bar").is_err());
        assert!(torrent.resolve("hello").is_err());
    }

    #[test]
    fn commands_are_filtered_by_name() {
        assert_eq!(commands_matching("TORR").len(), 1);
        assert_eq!(commands_matching("").len(), 3); // cloud providers have no palette commands
        assert!(commands_matching("zzz").is_empty());
    }
}
