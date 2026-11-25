//! Downloads files and folders from public MediaFire share links. MediaFire no longer hands out
//! API app IDs, so there is no account login: we read the direct link off the
//! share page, like other open-source MediaFire downloaders do.

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde_json::Value;
use std::path::PathBuf;
use url::Url;

use crate::{
    CollectionItem, DestinationKind, DownloadProgress, DownloadProvider, DownloadRequest,
    ProgressReporter, ProviderCommand, ResolvedDownload, direct_download,
};

const API: &str = "https://www.mediafire.com/api/1.5";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

#[derive(Debug, Default, Clone, Copy)]
pub struct MediaFire;

fn commands() -> &'static [ProviderCommand] {
    &[ProviderCommand {
        name: "mediafire",
        description: "Download a file or folder from a public MediaFire link",
        provider_id: "mediafire",
        input_prompt: "MediaFire file or folder link",
    }]
}

#[derive(Debug, PartialEq, Eq)]
enum Link {
    File { name: String },
    Folder { key: String, name: String },
}

/// Make a name from a URL or the API safe to use as one path component.
fn safe_name(raw: &str) -> String {
    let decoded = percent_decode_str(raw).decode_utf8_lossy();
    let cleaned: String = decoded
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '\0') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    match cleaned.trim() {
        "" | "." | ".." => "_".to_owned(),
        name => name.to_owned(),
    }
}

/// Understands `mediafire.com/file/<key>/<name>/file` and
/// `mediafire.com/folder/<key>/<name>`.
fn parse_link(input: &str) -> Result<Link, String> {
    let url = Url::parse(input).map_err(|_| "Enter a MediaFire file or folder link".to_owned())?;
    let host = url.host_str().unwrap_or_default();
    if host != "mediafire.com" && !host.ends_with(".mediafire.com") {
        return Err("Link must be on mediafire.com".to_owned());
    }
    let mut segments = url.path_segments().into_iter().flatten();
    let kind = segments.next().unwrap_or_default();
    let key = segments.next().filter(|key| !key.is_empty());
    let name = segments
        .next()
        .filter(|name| !name.is_empty() && *name != "file");
    match (kind, key) {
        ("file" | "view" | "download", _) => Ok(Link::File {
            name: safe_name(name.ok_or("Link must include the file name")?),
        }),
        ("folder", Some(key)) => Ok(Link::Folder {
            key: key.to_owned(),
            name: safe_name(name.unwrap_or(key)),
        }),
        _ => Err("Enter a MediaFire file or folder link".to_owned()),
    }
}

/// Pull the direct download URL out of a share page.
fn find_direct_link(html: &str) -> Option<String> {
    // Newer pages hide the link base64-encoded in `data-scrambled-url`.
    if let Some(encoded) = attribute_after(html, "data-scrambled-url=\"")
        && let Ok(bytes) = STANDARD.decode(encoded)
        && let Ok(link) = String::from_utf8(bytes)
        && link.starts_with("http")
    {
        return Some(link);
    }
    // Older pages put it straight in the download button's href.
    let button = html.find("id=\"downloadButton\"")?;
    let tag_start = html[..button].rfind("<a")?;
    let tag = &html[tag_start..button + html[button..].find('>')?];
    let link = attribute_after(tag, "href=\"")?;
    link.starts_with("http").then(|| link.to_owned())
}

fn attribute_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let end = text[start..].find('"')?;
    Some(&text[start..start + end])
}

#[async_trait::async_trait]
impl DownloadProvider for MediaFire {
    fn id(&self) -> &'static str {
        "mediafire"
    }

    fn name(&self) -> &'static str {
        "MediaFire"
    }

    fn commands(&self) -> &'static [ProviderCommand] {
        commands()
    }

    fn resolve(&self, input: &str) -> Result<ResolvedDownload, String> {
        Ok(match parse_link(input)? {
            Link::File { name } => ResolvedDownload {
                name,
                destination: DestinationKind::File,
            },
            Link::Folder { .. } => return Err("Pick files from the folder list instead".to_owned()),
        })
    }

    fn is_collection(&self, input: &str) -> bool {
        matches!(parse_link(input), Ok(Link::Folder { .. }))
    }

    async fn list_collection(&self, input: &str) -> Result<Vec<CollectionItem>> {
        let Link::Folder { key, .. } = parse_link(input).map_err(anyhow::Error::msg)? else {
            bail!("not a MediaFire folder link");
        };
        let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
        let mut items = list_tree(&client, &key).await?;
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(items)
    }

    async fn download(&self, request: DownloadRequest, progress: ProgressReporter) -> Result<()> {
        let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
        let link = direct_link(&client, &request.url).await?;
        direct_download::download_authorized(
            &link,
            None,
            &request.destination,
            request.paused,
            move |received, total| {
                progress(DownloadProgress {
                    received_bytes: received,
                    total_bytes: total,
                    torrent: None,
                });
            },
        )
        .await
    }
}

async fn direct_link(client: &reqwest::Client, page_url: &str) -> Result<String> {
    let html = client
        .get(page_url)
        .send()
        .await
        .context("opening the MediaFire page")?
        .error_for_status()
        .context("MediaFire returned an error for this link")?
        .text()
        .await?;
    find_direct_link(&html)
        .context("could not find a download link (is the file private or removed?)")
}

async fn get_content(client: &reqwest::Client, folder_key: &str, kind: &str) -> Result<Vec<Value>> {
    let mut items = Vec::new();
    for chunk in 1.. {
        let json: Value = client
            .get(format!("{API}/folder/get_content.php"))
            .query(&[
                ("folder_key", folder_key),
                ("content_type", kind),
                ("chunk", &chunk.to_string()),
                ("chunk_size", "1000"),
                ("response_format", "json"),
            ])
            .send()
            .await
            .context("listing the MediaFire folder")?
            .error_for_status()?
            .json()
            .await
            .context("parsing the MediaFire response")?;
        let response = &json["response"];
        if response["result"].as_str() != Some("Success") {
            let message = response["message"].as_str().unwrap_or("unknown error");
            bail!("MediaFire error: {message}");
        }
        let content = &response["folder_content"];
        items.extend(content[kind].as_array().cloned().unwrap_or_default());
        if content["more_chunks"].as_str() != Some("yes") {
            break;
        }
    }
    Ok(items)
}

/// Walk a folder tree (without recursion) and list every file in it.
async fn list_tree(client: &reqwest::Client, root_key: &str) -> Result<Vec<CollectionItem>> {
    let mut files = Vec::new();
    let mut pending = vec![(root_key.to_owned(), PathBuf::new())];
    while let Some((key, dir)) = pending.pop() {
        for folder in get_content(client, &key, "folders").await? {
            if let (Some(sub_key), Some(name)) =
                (folder["folderkey"].as_str(), folder["name"].as_str())
            {
                pending.push((sub_key.to_owned(), dir.join(safe_name(name))));
            }
        }
        for file in get_content(client, &key, "files").await? {
            let (Some(quick_key), Some(name)) =
                (file["quickkey"].as_str(), file["filename"].as_str())
            else {
                continue;
            };
            let name = safe_name(name);
            files.push(CollectionItem {
                name: dir.join(&name).display().to_string(),
                size: file["size"].as_str().and_then(|size| size.parse().ok()),
                // Re-uses the single-file path: only the key matters to MediaFire.
                input: format!(
                    "https://www.mediafire.com/file/{quick_key}/{}/file",
                    utf8_percent_encode(&name, NON_ALPHANUMERIC)
                ),
            });
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_parsed() {
        assert_eq!(
            parse_link("https://www.mediafire.com/file/abc123/my%20file.zip/file").unwrap(),
            Link::File {
                name: "my file.zip".to_owned()
            }
        );
        assert_eq!(
            parse_link("https://www.mediafire.com/folder/k3y/Photos").unwrap(),
            Link::Folder {
                key: "k3y".to_owned(),
                name: "Photos".to_owned()
            }
        );
        assert_eq!(
            parse_link("https://www.mediafire.com/folder/k3y").unwrap(),
            Link::Folder {
                key: "k3y".to_owned(),
                name: "k3y".to_owned()
            }
        );
        assert!(parse_link("https://example.com/file/abc/x").is_err());
        assert!(parse_link("nope").is_err());
    }

    #[test]
    fn names_cannot_escape_the_download_folder() {
        assert_eq!(safe_name(".."), "_");
        assert_eq!(safe_name("a%2F..%2Fb"), "a_.._b");
    }

    #[test]
    fn direct_link_is_found_in_either_page_style() {
        let scrambled = format!(
            "<a id=\"downloadButton\" href=\"javascript:void(0)\" data-scrambled-url=\"{}\">",
            STANDARD.encode("https://download1.mediafire.com/x/file.zip")
        );
        assert_eq!(
            find_direct_link(&scrambled).as_deref(),
            Some("https://download1.mediafire.com/x/file.zip")
        );
        let plain = "<a class=\"input\" aria-label=\"Download file\" href=\"https://download2.mediafire.com/y/f.zip\" id=\"downloadButton\">";
        assert_eq!(
            find_direct_link(plain).as_deref(),
            Some("https://download2.mediafire.com/y/f.zip")
        );
        assert!(find_direct_link("<html></html>").is_none());
    }
}
