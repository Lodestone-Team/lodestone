//! A server's resource pack (`resource-pack` in its `server.properties`), shown on the
//! Servers page, and "Download to this instance", which puts it into the instance's
//! `resourcepacks/` so it is there offline. Minecraft downloads server packs by itself
//! when it joins, so this is only a convenience.
//!
//! A pack is data, not code. The download is still checked: only `http`/`https` (as
//! Minecraft allows), at most [`MAX_PACK_BYTES`], the SHA-1 the server gives must
//! match, and the file must be a zip with `pack.mcmeta` at its root.

use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use serde::{Deserialize, Serialize};
use sha1::Digest as _;
use url::Url;

use super::{
    CoreClient, CoreError,
    client::{Auth, segment},
};
use crate::{Secret, USER_AGENT, worlds::folder_name};

/// Minecraft refuses server packs larger than this.
pub const MAX_PACK_BYTES: u64 = 250 * 1024 * 1024;
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_REDIRECTS: u32 = 5;

/// The resource pack lines of a server's `server.properties`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerResourcePack {
    pub url: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub prompt: String,
}

impl ServerResourcePack {
    /// The pack's host and file name, for showing: `example.com · pack.zip`.
    pub fn label(&self) -> String {
        match Url::parse(&self.url) {
            Ok(url) => {
                let file = url
                    .path_segments()
                    .and_then(|mut s| s.next_back())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("pack");
                format!("{} · {file}", url.host_str().unwrap_or_default())
            }
            Err(_) => self.url.clone(),
        }
    }
}

impl CoreClient {
    /// The server's resource pack, or `None` when it has none (or the core is too old to
    /// say). Any key that can view the server may read it.
    pub async fn resource_pack(
        &self,
        key: &Secret,
        uuid: &str,
    ) -> Result<Option<ServerResourcePack>, CoreError> {
        let pack: Result<ServerResourcePack, CoreError> = self
            .json(
                Method::GET,
                &format!("instance/{}/resource_pack", segment(uuid)?),
                Auth::Bearer(key),
                None,
            )
            .await;
        match pack {
            Ok(pack) if pack.url.trim().is_empty() => Ok(None),
            Ok(pack) => Ok(Some(pack)),
            Err(CoreError::NotFound(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

/// The pack's URL, if the launcher may download it.
pub fn check_url(url: &str) -> Result<Url, String> {
    let url = Url::parse(url.trim()).map_err(|_| "The pack's URL isn't valid.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("The pack's URL must start with http:// or https://.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("The pack's URL has a username or password in it.".into());
    }
    Ok(url)
}

/// Whether the zip at `path` is a resource pack: `pack.mcmeta` at its root.
pub fn is_resource_pack(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(std::io::BufReader::new(file)) else {
        return false;
    };
    archive.by_name("pack.mcmeta").is_ok()
}

fn sha1_of(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// The file the pack of `server_name` is saved as: `<server name> (server).zip`.
pub fn file_name(server_name: &str) -> String {
    format!("{} (server).zip", folder_name(server_name))
}

/// Downloads `pack` into `resourcepacks` (made if missing). A file there with the same
/// SHA-1 is kept as it is. Returns the pack's path.
pub async fn download(
    http: Arc<dyn HttpClient>,
    pack: &ServerResourcePack,
    server_name: &str,
    resourcepacks: &Path,
) -> Result<PathBuf, String> {
    let url = check_url(&pack.url)?;
    let expected = pack.sha1.trim().to_ascii_lowercase();
    if !expected.is_empty()
        && (expected.len() != 40 || !expected.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("The server's SHA-1 for the pack isn't valid.".into());
    }
    fs::create_dir_all(resourcepacks).map_err(|e| format!("{}: {e}", resourcepacks.display()))?;
    let target = resourcepacks.join(file_name(server_name));
    if target.is_file() && !expected.is_empty() && sha1_of(&target).as_deref() == Some(&expected) {
        return Ok(target);
    }
    let request = Request::builder()
        .method(Method::GET)
        .uri(url.as_str())
        .header("User-Agent", USER_AGENT)
        .follow_redirects(RedirectPolicy::FollowLimit(MAX_REDIRECTS))
        .timeout(DOWNLOAD_TIMEOUT)
        .body(AsyncBody::empty())
        .map_err(|e| e.to_string())?;
    let response = http
        .send(request)
        .await
        .map_err(|e| format!("Couldn't download the pack: {e:#}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("The pack's URL answered {status}."));
    }
    let partial = resourcepacks.join(format!(
        ".lodestone-pack-{:016x}.part",
        u64::from_le_bytes(crate::random_bytes())
    ));
    let result = async {
        let mut body = response.into_body();
        let mut out = File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
        let mut hasher = sha1::Sha1::new();
        let mut buffer = vec![0u8; 256 * 1024];
        let mut size = 0u64;
        loop {
            let read = body
                .read(&mut buffer)
                .await
                .map_err(|e| format!("The download stopped: {e}"))?;
            if read == 0 {
                break;
            }
            size += read as u64;
            if size > MAX_PACK_BYTES {
                return Err(format!(
                    "The pack is larger than {} MB.",
                    MAX_PACK_BYTES / (1024 * 1024)
                ));
            }
            hasher.update(&buffer[..read]);
            out.write_all(&buffer[..read])
                .map_err(|e| format!("{}: {e}", partial.display()))?;
        }
        out.sync_all()
            .map_err(|e| format!("{}: {e}", partial.display()))?;
        drop(out);
        let sha1 = format!("{:x}", hasher.finalize());
        if !expected.is_empty() && sha1 != expected {
            return Err(
                "The downloaded pack doesn't match the server's SHA-1, so it wasn't saved.".into(),
            );
        }
        if !is_resource_pack(&partial) {
            return Err("The download isn't a resource pack (no pack.mcmeta).".into());
        }
        fs::rename(&partial, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(target.clone())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}
