use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::{Account, CloudFile, CloudProvider, OpenUrl, oauth};
use crate::{
    DestinationKind, DownloadProgress, DownloadProvider, DownloadRequest, ProgressReporter,
    ProviderCommand, ResolvedDownload, direct_download,
};

const ENDPOINTS: oauth::OAuthEndpoints = oauth::OAuthEndpoints {
    auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
    scope: "https://www.googleapis.com/auth/drive.readonly",
};
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

static ACCESS_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::const_new(None);

#[derive(Debug, Default, Clone, Copy)]
pub struct GoogleDrive;

#[derive(Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<ApiFile>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct ApiFile {
    id: String,
    name: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
    size: Option<String>,
}

async fn access_token(account: &Account) -> Result<String> {
    let mut cached = ACCESS_TOKEN.lock().await;
    if let Some((token, expires)) = cached.as_ref()
        && *expires > Instant::now()
    {
        return Ok(token.clone());
    }
    let Some(refresh_token) = account.refresh_token.as_deref() else {
        bail!("not logged in");
    };
    let response = oauth::token_request(
        TOKEN_URL,
        &[
            ("client_id", &account.client_id),
            ("client_secret", &account.client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
    )
    .await?;
    let expires = Instant::now() + Duration::from_secs(response.expires_in.saturating_sub(60));
    *cached = Some((response.access_token.clone(), expires));
    Ok(response.access_token)
}

pub fn download_input(file: &CloudFile) -> String {
    format!("gdrive://{}/{}", file.id, file.name.replace('/', "_"))
}

#[async_trait::async_trait]
impl DownloadProvider for GoogleDrive {
    fn id(&self) -> &'static str {
        "gdrive"
    }

    fn name(&self) -> &'static str {
        "Google Drive"
    }

    fn commands(&self) -> &'static [ProviderCommand] {
        &[] // started from the provider screen, not the command palette
    }

    fn resolve(&self, input: &str) -> Result<ResolvedDownload, String> {
        let (id, name) = input
            .strip_prefix("gdrive://")
            .and_then(|rest| rest.split_once('/'))
            .filter(|(id, name)| !id.is_empty() && !name.is_empty())
            .ok_or_else(|| "Invalid Google Drive file reference".to_owned())?;
        let _ = id;
        Ok(ResolvedDownload {
            name: name.to_owned(),
            destination: DestinationKind::File,
        })
    }

    async fn download(&self, request: DownloadRequest, progress: ProgressReporter) -> Result<()> {
        let id = request
            .url
            .strip_prefix("gdrive://")
            .and_then(|rest| rest.split('/').next())
            .context("invalid Google Drive file reference")?;
        let account = Account::load(CloudProvider::id(self))?;
        let token = access_token(&account).await?;
        let url = format!("{FILES_URL}/{id}?alt=media");
        direct_download::download_authorized(
            &url,
            Some(&token),
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

#[async_trait::async_trait]
impl CloudProvider for GoogleDrive {
    fn id(&self) -> &'static str {
        "gdrive"
    }

    fn name(&self) -> &'static str {
        "Google Drive"
    }

    async fn login(&self, account: &Account, open_url: OpenUrl) -> Result<String> {
        if !account.has_client() {
            bail!("set the client ID and client secret first");
        }
        let login = oauth::LoopbackLogin::start(&ENDPOINTS, &account.client_id).await?;
        open_url(login.authorize_url.clone());
        let code = login.wait_for_code().await?;
        let response = oauth::token_request(
            TOKEN_URL,
            &[
                ("client_id", &account.client_id),
                ("client_secret", &account.client_secret),
                ("code", &code),
                ("code_verifier", &login.verifier),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &login.redirect_uri),
            ],
        )
        .await?;
        *ACCESS_TOKEN.lock().await = None;
        response
            .refresh_token
            .context("Google did not return a refresh token")
    }

    async fn list(&self, account: &Account, folder: Option<&str>) -> Result<Vec<CloudFile>> {
        let token = access_token(account).await?;
        let client = reqwest::Client::new();
        let query = format!(
            "'{}' in parents and trashed = false",
            folder.unwrap_or("root")
        );
        let mut files = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut request = client.get(FILES_URL).bearer_auth(&token).query(&[
                ("q", query.as_str()),
                ("fields", "nextPageToken,files(id,name,mimeType,size)"),
                ("orderBy", "folder,name"),
                ("pageSize", "1000"),
            ]);
            if let Some(page) = &page_token {
                request = request.query(&[("pageToken", page.as_str())]);
            }
            let response = request.send().await.context("listing Google Drive")?;
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                bail!("Google Drive returned {status}: {body}");
            }
            let page: FileList = response.json().await.context("parsing the file list")?;
            files.extend(page.files.into_iter().map(|file| CloudFile {
                is_folder: file.mime_type == FOLDER_MIME,
                downloadable: !file.mime_type.starts_with("application/vnd.google-apps."),
                size: file.size.and_then(|size| size.parse().ok()),
                id: file.id,
                name: file.name,
            }));
            match page.next_page_token {
                Some(next) => page_token = Some(next),
                None => break,
            }
        }
        Ok(files)
    }
}
