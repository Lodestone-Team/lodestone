//! A server's world in a launcher instance: the zip a core makes
//! (`GET /instance/{uuid}/world/download`), imported into `saves/` as a singleplayer
//! world with the world import of [`crate::worlds`] (every path checked, links
//! skipped, a new folder that doesn't clash).
//!
//! The zip has the world's folder (`world/`), and on Bukkit-style servers (Paper,
//! Spigot) the other dimensions beside it: `world_nether/DIM-1/` and
//! `world_the_end/DIM1/`. Singleplayer keeps those inside the world folder, so they
//! are moved there; the rest of those folders (their own `level.dat`, `uid.dat`) is
//! left out.

use std::{
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpRequestExt as _, Method, RedirectPolicy, Request};
use lodestone_minecraft::level::{self, LevelSummary, MAX_LEVEL_DAT_BYTES};
use serde::{Deserialize, Serialize};

use super::{
    CoreClient, CoreError,
    client::{Auth, MAX_JSON_BYTES, RawResponse, check, segment},
};
use crate::{
    Secret, USER_AGENT,
    content::{InstanceDirs, is_plain_name},
    game::download::{CancelToken, Progress, ProgressSnapshot},
    worlds::{
        MAX_ZIP_BYTES, MAX_ZIP_ENTRIES, WorldError, extract_mapped, folder_name, is_mac_metadata,
        saves_dir, set_level_name, unused,
    },
};

/// What the core says about a world download.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldDownload {
    /// For `GET /file/{key}`.
    pub key: String,
    pub file_name: String,
    /// The world's folder in the zip.
    pub folder: String,
    /// The other dimensions' folders, such as `world_nether`.
    #[serde(default)]
    pub dimension_folders: Vec<String>,
    #[serde(default)]
    pub size: u64,
}

/// Where each dimension of a Bukkit-style server is, and where singleplayer keeps it.
const DIMENSIONS: [(&str, &str); 2] = [("_nether", "DIM-1"), ("_the_end", "DIM1")];

/// The world in a server's zip, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerWorldZip {
    pub summary: LevelSummary,
    /// `(zip prefix, folder in the world)` for [`extract_mapped`].
    pub mappings: Vec<(String, String)>,
}

fn invalid(message: impl Into<String>) -> WorldError {
    WorldError::Invalid(message.into())
}

/// Opens a server's world zip and checks it: its size, the world folder's
/// `level.dat`, and which dimension folders to move into the world.
pub fn inspect(path: &Path, download: &WorldDownload) -> Result<ServerWorldZip, WorldError> {
    if !is_plain_name(&download.folder) {
        return Err(invalid(
            "The core named a world folder that isn't a plain name.",
        ));
    }
    let file = File::open(path).map_err(|source| WorldError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| {
        invalid(format!(
            "The world download isn't a zip that can be read: {err}"
        ))
    })?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(invalid(format!(
            "The zip has more than {MAX_ZIP_ENTRIES} files."
        )));
    }
    let world = format!("{}/", download.folder);
    let mut total = 0u64;
    let mut names = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|err| invalid(format!("The zip can't be read: {err}")))?;
        total = total.saturating_add(entry.size());
        names.push(entry.name().to_string());
    }
    if total > MAX_ZIP_BYTES {
        return Err(invalid("The zip's files are larger than 64 GB unpacked."));
    }
    let level_dat = format!("{world}level.dat");
    let Some(index) = names.iter().position(|n| *n == level_dat) else {
        return Err(invalid(format!(
            "The download has no {level_dat}, so it isn't a world."
        )));
    };
    let mut entry = archive
        .by_index(index)
        .map_err(|err| invalid(format!("The zip can't be read: {err}")))?;
    if entry.size() > MAX_LEVEL_DAT_BYTES {
        return Err(invalid("Its level.dat is too large."));
    }
    let mut bytes = Vec::new();
    (&mut entry)
        .take(MAX_LEVEL_DAT_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(format!("Its level.dat can't be read: {e}")))?;
    let summary = level::summarize(&bytes)
        .map_err(|e| invalid(format!("Its level.dat can't be read: {e}")))?;

    let mut mappings = vec![(world.clone(), String::new())];
    for (suffix, dimension) in DIMENSIONS {
        let sibling = format!("{}{suffix}", download.folder);
        if !download.dimension_folders.contains(&sibling) {
            continue;
        }
        // a world that has the dimension inside already keeps its own
        let inside = format!("{world}{dimension}/");
        if names.iter().any(|n| n.starts_with(&inside)) {
            continue;
        }
        let source = format!("{sibling}/{dimension}/");
        if names
            .iter()
            .any(|n| n.starts_with(&source) && !is_mac_metadata(n))
        {
            mappings.push((source, format!("{dimension}/")));
        }
    }
    Ok(ServerWorldZip { summary, mappings })
}

/// Imports a server's world zip into the instance's `saves/`, as a new world named
/// `name` (the server's name). Returns the new folder.
pub fn import(
    dirs: &InstanceDirs,
    path: &Path,
    download: &WorldDownload,
    name: &str,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<String, WorldError> {
    let world = inspect(path, download)?;
    let saves = saves_dir(dirs);
    let folder = unused(&saves, &folder_name(name));
    let dir: PathBuf = extract_mapped(path, &world.mappings, &saves, &folder, progress, cancel)?;
    // the server's level.dat calls it by its folder name (`world`)
    set_level_name(&dir, name);
    Ok(folder)
}

/// How long the core may take to zip a world.
pub const ZIP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
/// How long a world download may take.
pub const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3 * 60 * 60);

impl CoreClient {
    /// Asks the core to zip a server's world. Needs a session: the device key can't read
    /// a server's files. Fails with a conflict while the server runs.
    pub async fn world_download(
        &self,
        session: &Secret,
        uuid: &str,
    ) -> Result<WorldDownload, CoreError> {
        let response = self
            .send_typed(
                Method::GET,
                &format!("instance/{}/world/download", segment(uuid)?),
                Auth::Bearer(session),
                None,
                None,
                MAX_JSON_BYTES,
                ZIP_TIMEOUT,
            )
            .await?;
        check(&response, Auth::Bearer(session))?;
        serde_json::from_slice(&response.body).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// Downloads `GET /file/{key}` into `out` (created, or replaced), at most `limit`
    /// bytes. The key itself is the permission, so no token is sent.
    pub async fn download_file(
        &self,
        key: &str,
        out: &Path,
        limit: u64,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<u64, CoreError> {
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(CoreError::Invalid("the download key isn't valid".into()));
        }
        let url = self.url(&format!("file/{key}"));
        // through the core's pinned certificate, like every other request
        let response = self
            .send_checked(|| {
                Request::builder()
                    .method(Method::GET)
                    .uri(url.as_str())
                    .header("User-Agent", USER_AGENT)
                    .follow_redirects(RedirectPolicy::NoFollow)
                    .timeout(DOWNLOAD_TIMEOUT)
                    .body(AsyncBody::empty())
                    .map_err(|err| CoreError::InvalidAddress(err.to_string()))
            })
            .await?;
        let status = response.status().as_u16();
        let total = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let mut body = response.into_body();
        if !(200..300).contains(&status) {
            let mut text = Vec::new();
            let _ = (&mut body).take(64 * 1024).read_to_end(&mut text).await;
            check(
                &RawResponse {
                    status,
                    etag: None,
                    body: text,
                },
                Auth::None,
            )?;
        }
        if total.is_some_and(|t| t > limit) {
            return Err(CoreError::Invalid(format!(
                "the download is larger than {} GB",
                limit / (1024 * 1024 * 1024)
            )));
        }
        let io = |err: std::io::Error| CoreError::Invalid(format!("{}: {err}", out.display()));
        let mut file = File::create(out).map_err(io)?;
        let mut buffer = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::Invalid("cancelled".into()));
            }
            let read = body
                .read(&mut buffer)
                .await
                .map_err(|err| CoreError::Unreachable(err.to_string()))?;
            if read == 0 {
                break;
            }
            done += read as u64;
            if done > limit {
                return Err(CoreError::Invalid(
                    "the download is larger than it may be".into(),
                ));
            }
            std::io::Write::write_all(&mut file, &buffer[..read]).map_err(io)?;
            progress.set(ProgressSnapshot {
                files_total: 1,
                files_done: 0,
                bytes_total: total.unwrap_or(0),
                bytes_done: done,
            });
        }
        file.sync_all().map_err(io)?;
        if total.is_some_and(|t| t != done) {
            return Err(CoreError::Unreachable("the download stopped early".into()));
        }
        Ok(done)
    }
}
