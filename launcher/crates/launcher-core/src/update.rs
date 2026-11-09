//! Updating the launcher from its GitHub releases.
//!
//! Each launcher version is a GitHub release of its own, tagged `launcher-vX.Y.Z`
//! ([`TAG_PREFIX`]), or `launcher-vX.Y.Z-beta.N` for a beta, which GitHub marks as a
//! pre-release. CI and developers upload each platform's build to it while it is a
//! draft (`launcher/scripts/publish_release.py`), with a `latest.json` [`Manifest`]
//! listing each platform's file with its build number, commit, size and SHA-256, and
//! a developer publishes it when it's ready. Drafts can't be seen from outside.
//!
//! A check lists the repository's releases ([`RELEASES_URL`], GitHub's API) and
//! takes the published `launcher-v*` one whose file for this platform has the
//! highest build ([`pick_release`]): stable releases only, or pre-releases too with
//! Settings > General > "Beta builds" on ([`UpdateChannel`]), so a beta launcher is
//! never behind stable. The build is in each file's name, so one listing is enough;
//! then that release's `latest.json` gives the file's size and SHA-256. Leaving the
//! beta keeps the running build until a stable one is newer: updates never go back.
//!
//! A build's number is the count of commits in its history (`git rev-list --count`).
//! The script compiles it in as `LODESTONE_LAUNCHER_BUILD`; a plain `cargo build`
//! has none, and never updates itself.
//!
//! Launchers from before versioned releases read `launcher-latest/latest.json`;
//! `publish_release.py --bridge` copies a version there so they update to one that
//! reads releases.
//!
//! An update is downloaded next to the running executable, checked against the
//! manifest, and swapped in for it ([`install`]): on Unix by renaming over it, on
//! Windows by first renaming the running executable aside (Windows allows that, but
//! not replacing it), to `<exe>.old`, which the next start removes ([`clean_up`]).
//! The process keeps running the old build until it restarts ([`restart`]); a plain
//! quit and start also runs the new one.
//!
//! Lodestone Host updates the same way, from its own releases ([`Product::HOST`]:
//! `host-vX.Y.Z`, `lodestone-host-<build>-<platform>-standalone`). Its manifest also
//! lists the Lodestone Core program it runs ([`Asset::core`]), which is installed next
//! to it.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::USER_AGENT;
pub use crate::settings::UpdateChannel;

/// The repository's newest releases, of every product: the launcher's are the ones
/// tagged [`TAG_PREFIX`]. The newest 100 (GitHub's most per page) reach back past
/// any run of betas to the newest stable release.
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/Lodestone-Team/lodestone/releases?per_page=100";

/// The start of a launcher version's tag. The release workflow checks a commit has
/// this line before releasing it, so it never releases a launcher that updates the
/// old way.
pub const TAG_PREFIX: &str = "launcher-v";

/// The release manifest's file name in each release.
pub const MANIFEST_NAME: &str = "latest.json";

/// Overrides [`RELEASES_URL`], to test against a local server that answers like
/// GitHub's API.
pub const RELEASES_URL_ENV: &str = "LODESTONE_LAUNCHER_RELEASES_URL";

/// A manifest's address that replaces the release lookup altogether, to test
/// updating against a folder `publish_release.py --output` made.
pub const MANIFEST_URL_ENV: &str = "LODESTONE_LAUNCHER_UPDATE_URL";

/// The argument a restarted launcher gets: the process id of the launcher that
/// started it, which it waits to exit before it opens its window.
pub const AFTER_UPDATE_ARG: &str = "--after-update";

/// The manifest is a few hundred bytes; anything far larger is not one.
const MAX_MANIFEST_BYTES: usize = 256 * 1024;

/// GitHub's list of 100 releases, notes and files included, is around a MB.
const MAX_RELEASES_BYTES: usize = 16 * 1024 * 1024;

/// A launcher binary is tens of MB. A larger file is refused before it is read.
const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;

/// How long a restarted launcher waits for the old one to exit.
const WAIT_FOR_PREVIOUS: Duration = Duration::from_secs(15);

/// The start of a Lodestone Host version's tag.
pub const HOST_TAG_PREFIX: &str = "host-v";

/// Which app is updating: each has its own releases, file names and test overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Product {
    /// The start of its versions' tags: `launcher-v`.
    pub tag_prefix: &'static str,
    /// The start of its files' names: `lodestone-launcher-`.
    pub file_prefix: &'static str,
    /// Overrides [`RELEASES_URL`] for it.
    pub releases_url_env: &'static str,
    /// Replaces the release lookup with a manifest's address, for it.
    pub manifest_url_env: &'static str,
}

impl Product {
    /// Lodestone Launcher.
    pub const LAUNCHER: Product = Product {
        tag_prefix: TAG_PREFIX,
        file_prefix: "lodestone-launcher-",
        releases_url_env: RELEASES_URL_ENV,
        manifest_url_env: MANIFEST_URL_ENV,
    };

    /// Lodestone Host.
    pub const HOST: Product = Product {
        tag_prefix: HOST_TAG_PREFIX,
        file_prefix: "lodestone-host-",
        releases_url_env: "LODESTONE_HOST_RELEASES_URL",
        manifest_url_env: "LODESTONE_HOST_UPDATE_URL",
    };

    /// The build of `name` if it is `platform`'s standalone binary:
    /// `<file_prefix><build>-<platform>-standalone[.exe]`.
    pub fn asset_build(&self, name: &str, platform: &str) -> Option<u64> {
        let rest = name.strip_prefix(self.file_prefix)?;
        let (build, rest) = rest.split_once('-')?;
        let rest = rest.strip_prefix(platform)?;
        if !matches!(rest, "-standalone" | "-standalone.exe") || build.is_empty() {
            return None;
        }
        if !build.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        build.parse().ok()
    }
}

/// This build's number and commit, as the publish script compiled them in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Build {
    pub number: u64,
    pub commit: Option<&'static str>,
}

impl Build {
    /// This build, or `None` for a build the publish script didn't make, which
    /// doesn't update.
    pub fn current() -> Option<Build> {
        Self::parse(
            option_env!("LODESTONE_LAUNCHER_BUILD"),
            option_env!("LODESTONE_LAUNCHER_COMMIT"),
        )
    }

    /// A build from the values the publish script compiled in, such as
    /// `option_env!("LODESTONE_HOST_BUILD")`; `None` without a number.
    pub fn parse(number: Option<&str>, commit: Option<&'static str>) -> Option<Build> {
        let number = number?.trim().parse().ok()?;
        let commit = commit.map(str::trim).filter(|commit| !commit.is_empty());
        Some(Build { number, commit })
    }

    /// The commit's first seven characters, as GitHub shows it.
    pub fn short_commit(&self) -> Option<&'static str> {
        self.commit.map(short_commit)
    }
}

/// A commit hash's first seven characters.
pub fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// The key of this platform's file in the manifest: `windows-x86_64`,
/// `macos-aarch64`, `linux-x86_64`.
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// `latest.json`: the newest published build of each platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The newest build of any platform. A platform's own is [`Asset::build`].
    pub build: u64,
    pub commit: String,
    /// The workspace version the build was made from.
    #[serde(default)]
    pub version: String,
    /// When it was published, RFC 3339.
    #[serde(default)]
    pub published: Option<String>,
    /// The release's tag, `launcher-v0.2.0`. Missing in manifests from before
    /// versioned releases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// By [`platform`] key.
    pub assets: BTreeMap<String, Asset>,
}

/// One platform's binary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// The file name, next to `latest.json` in the release.
    pub name: String,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: String,
    /// This file's build number; [`Manifest::build`] if missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Lodestone Host's: the Lodestone Core program it runs, next to it in the
    /// release, of the same build. It is installed next to the Host's executable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core: Option<Companion>,
}

/// A second file that goes with a platform's binary ([`Asset::core`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Companion {
    pub name: String,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: String,
}

impl Companion {
    /// It as an [`Asset`], to [`download`] it.
    pub fn as_asset(&self) -> Asset {
        Asset {
            name: self.name.clone(),
            size: self.size,
            sha256: self.sha256.clone(),
            build: None,
            commit: None,
            core: None,
        }
    }
}

impl Manifest {
    /// This platform's binary and its build number.
    pub fn for_platform(&self) -> Option<(u64, &Asset)> {
        let asset = self.assets.get(&platform())?;
        Some((asset.build.unwrap_or(self.build), asset))
    }

    /// This platform's binary and its build number, if that build is newer than
    /// `than`.
    pub fn newer(&self, than: u64) -> Option<(u64, &Asset)> {
        self.for_platform().filter(|(build, _)| *build > than)
    }

    /// Where `asset` is: next to the manifest at `manifest_url`.
    pub fn asset_url(manifest_url: &str, asset: &Asset) -> Result<String, UpdateError> {
        let base = url::Url::parse(manifest_url)
            .map_err(|err| UpdateError::Manifest(format!("{manifest_url}: {err}")))?;
        let url = base
            .join(&asset.name)
            .map_err(|err| UpdateError::Manifest(format!("{}: {err}", asset.name)))?;
        check_url(url.as_str())?;
        Ok(url.into())
    }
}

/// Why checking for or installing an update failed.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("couldn't reach GitHub: {0}")]
    Network(String),
    #[error("GitHub answered HTTP {0}")]
    Status(u16),
    #[error("GitHub is limiting how often this network checks for updates; trying again later")]
    RateLimited,
    #[error("the update information isn't valid: {0}")]
    Manifest(String),
    #[error("the download doesn't match the published file: {0}")]
    Mismatch(String),
    #[error("couldn't write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl UpdateError {
    fn io(path: &Path, source: io::Error) -> Self {
        UpdateError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

fn env_url(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|url| !url.trim().is_empty())
}

/// A manifest and the URL it was read from, which its files are next to
/// ([`Manifest::asset_url`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub url: String,
    pub manifest: Manifest,
    /// The release it is in, `launcher-v0.2.0`; `None` for [`MANIFEST_URL_ENV`].
    pub tag: Option<String>,
}

impl Found {
    /// This platform's build in it, or 0 if it has none.
    pub fn build(&self) -> u64 {
        self.manifest.for_platform().map_or(0, |(build, _)| build)
    }
}

/// A release, as GitHub's API lists it: only what choosing one needs.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// The release a check settled on, before its manifest is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub tag: String,
    /// The build of its file for this platform.
    pub build: u64,
    pub manifest_url: String,
}

/// The build of `name` if it is `platform`'s standalone launcher binary:
/// `lodestone-launcher-<build>-<platform>-standalone[.exe]`.
pub fn asset_build(name: &str, platform: &str) -> Option<u64> {
    Product::LAUNCHER.asset_build(name, platform)
}

/// Of `releases`, the published launcher release that `channel` reads with the
/// highest build for `platform`, and that has a manifest. On a tie, the first
/// (GitHub lists the newest first).
pub fn pick_release(
    releases: &[Release],
    channel: UpdateChannel,
    platform: &str,
) -> Option<Candidate> {
    pick_release_for(Product::LAUNCHER, releases, channel, platform)
}

/// [`pick_release`] for `product`'s releases.
pub fn pick_release_for(
    product: Product,
    releases: &[Release],
    channel: UpdateChannel,
    platform: &str,
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    for release in releases {
        if !release.tag_name.starts_with(product.tag_prefix)
            || release.draft
            || (release.prerelease && channel != UpdateChannel::Beta)
        {
            continue;
        }
        let Some(manifest) = release.assets.iter().find(|a| a.name == MANIFEST_NAME) else {
            continue;
        };
        let Some(build) = release
            .assets
            .iter()
            .filter_map(|asset| product.asset_build(&asset.name, platform))
            .max()
        else {
            continue;
        };
        if best.as_ref().is_none_or(|best| build > best.build) {
            best = Some(Candidate {
                tag: release.tag_name.clone(),
                build,
                manifest_url: manifest.browser_download_url.clone(),
            });
        }
    }
    best
}

/// Looks up the newest build `channel` offers: the release to take (or `None` if
/// none has a build for this platform) and its manifest. Run it inside
/// [`crate::http::in_http_runtime`].
pub async fn find_update(
    http: &dyn HttpClient,
    channel: UpdateChannel,
) -> Result<Option<Found>, UpdateError> {
    find_update_for(Product::LAUNCHER, http, channel).await
}

/// [`find_update`] for `product`'s releases.
pub async fn find_update_for(
    product: Product,
    http: &dyn HttpClient,
    channel: UpdateChannel,
) -> Result<Option<Found>, UpdateError> {
    if let Some(url) = env_url(product.manifest_url_env) {
        let manifest = fetch_manifest(http, &url).await?;
        let tag = manifest.tag.clone();
        return Ok(Some(Found { url, manifest, tag }));
    }
    let list = env_url(product.releases_url_env).unwrap_or_else(|| RELEASES_URL.to_string());
    let releases = fetch_releases(http, &list).await?;
    let Some(candidate) = pick_release_for(product, &releases, channel, &platform()) else {
        return Ok(None);
    };
    let manifest = fetch_manifest(http, &candidate.manifest_url).await?;
    match manifest.for_platform() {
        Some((build, _)) if build == candidate.build => {}
        listed => {
            return Err(UpdateError::Manifest(format!(
                "{}'s {MANIFEST_NAME} lists build {}, but its file is build {}",
                candidate.tag,
                listed.map_or(0, |(build, _)| build),
                candidate.build
            )));
        }
    }
    Ok(Some(Found {
        url: candidate.manifest_url,
        manifest,
        tag: Some(candidate.tag),
    }))
}

/// Lists releases from GitHub's API.
async fn fetch_releases(http: &dyn HttpClient, url: &str) -> Result<Vec<Release>, UpdateError> {
    check_url(url)?;
    let request = Request::builder()
        .method(Method::GET)
        .uri(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .follow_redirects(RedirectPolicy::FollowAll)
        .body(AsyncBody::empty())
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let response = http
        .send(request)
        .await
        .map_err(|err| UpdateError::Network(format!("{err:#}")))?;
    let status = response.status().as_u16();
    // Without signing in, GitHub's API allows 60 requests an hour per address.
    if status == 403 || status == 429 {
        return Err(UpdateError::RateLimited);
    }
    if !(200..300).contains(&status) {
        return Err(UpdateError::Status(status));
    }
    let mut body = Vec::new();
    response
        .into_body()
        .take(MAX_RELEASES_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .await
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    if body.len() > MAX_RELEASES_BYTES {
        return Err(UpdateError::Manifest(
            "the list of releases is too large".into(),
        ));
    }
    serde_json::from_slice(&body)
        .map_err(|err| UpdateError::Manifest(format!("the list of releases: {err}")))
}

/// Updates are fetched over HTTPS; plain HTTP only from this machine, for tests.
fn check_url(url: &str) -> Result<(), UpdateError> {
    let parsed = url::Url::parse(url).map_err(|err| UpdateError::Manifest(err.to_string()))?;
    let loopback = matches!(parsed.host(), Some(url::Host::Domain("localhost")))
        || matches!(parsed.host(), Some(url::Host::Ipv4(ip)) if ip.is_loopback())
        || matches!(parsed.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback());
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if loopback => Ok(()),
        _ => Err(UpdateError::Manifest(format!(
            "{url} isn't an HTTPS address"
        ))),
    }
}

/// A `GET` that follows GitHub's redirect from the release to its file storage.
fn request(url: &str, accept: &str) -> Result<Request<AsyncBody>, UpdateError> {
    check_url(url)?;
    Request::builder()
        .method(Method::GET)
        .uri(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", accept)
        .follow_redirects(RedirectPolicy::FollowAll)
        .body(AsyncBody::empty())
        .map_err(|err| UpdateError::Network(err.to_string()))
}

/// Fetches the manifest. Run it inside [`crate::http::in_http_runtime`].
pub async fn fetch_manifest(http: &dyn HttpClient, url: &str) -> Result<Manifest, UpdateError> {
    let response = http
        .send(request(url, "application/json")?)
        .await
        .map_err(|err| UpdateError::Network(format!("{err:#}")))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdateError::Status(status));
    }
    let mut body = Vec::new();
    response
        .into_body()
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .await
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    if body.len() > MAX_MANIFEST_BYTES {
        return Err(UpdateError::Manifest("it is too large".into()));
    }
    parse_manifest(&body)
}

fn parse_manifest(body: &[u8]) -> Result<Manifest, UpdateError> {
    let manifest: Manifest =
        serde_json::from_slice(body).map_err(|err| UpdateError::Manifest(err.to_string()))?;
    for asset in manifest.assets.values() {
        check_file(&asset.name, asset.size, &asset.sha256)?;
        if let Some(core) = &asset.core {
            check_file(&core.name, core.size, &core.sha256)?;
        }
    }
    Ok(manifest)
}

/// A file a manifest lists: a plain name, a SHA-256 and a believable size.
fn check_file(name: &str, size: u64, sha256: &str) -> Result<(), UpdateError> {
    // Only a plain file name, so it can't point outside the release.
    let plain =
        !name.is_empty() && !name.contains(['/', '\\', '?', '#']) && name != "." && name != "..";
    if !plain {
        return Err(UpdateError::Manifest(format!("bad file name {name:?}")));
    }
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UpdateError::Manifest(format!("bad SHA-256 for {name}")));
    }
    if size == 0 || size > MAX_BINARY_BYTES {
        return Err(UpdateError::Manifest(format!("bad size for {name}")));
    }
    Ok(())
}

/// Downloads `asset` from `url` to `dest`, calling `progress` with the bytes read so
/// far. The file only appears at `dest` once its size and SHA-256 match. Run it
/// inside [`crate::http::in_http_runtime`].
pub async fn download(
    http: &dyn HttpClient,
    url: &str,
    asset: &Asset,
    dest: &Path,
    progress: impl Fn(u64),
) -> Result<(), UpdateError> {
    let response = http
        .send(request(url, "application/octet-stream")?)
        .await
        .map_err(|err| UpdateError::Network(format!("{err:#}")))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdateError::Status(status));
    }
    let part = with_suffix(dest, ".part");
    let mut file = File::create(&part).map_err(|err| UpdateError::io(&part, err))?;
    let result = async {
        let mut body = response.into_body();
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            let read = body
                .read(&mut buffer)
                .await
                .map_err(|err| UpdateError::Network(err.to_string()))?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > asset.size {
                return Err(UpdateError::Mismatch(format!(
                    "it is larger than {} bytes",
                    asset.size
                )));
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])
                .map_err(|err| UpdateError::io(&part, err))?;
            progress(written);
        }
        if written != asset.size {
            return Err(UpdateError::Mismatch(format!(
                "it is {written} bytes, not {}",
                asset.size
            )));
        }
        let hash = format!("{:x}", hasher.finalize());
        if !hash.eq_ignore_ascii_case(&asset.sha256) {
            return Err(UpdateError::Mismatch(format!(
                "its SHA-256 is {hash}, not {}",
                asset.sha256
            )));
        }
        file.sync_all().map_err(|err| UpdateError::io(&part, err))?;
        Ok(())
    }
    .await;
    drop(file);
    match result {
        Ok(()) => crate::fsx::rename(&part, dest).map_err(|err| {
            let _ = fs::remove_file(&part);
            UpdateError::io(dest, err)
        }),
        Err(err) => {
            let _ = fs::remove_file(&part);
            Err(err)
        }
    }
}

/// The running executable's path, as it was when the process started: on Windows
/// the file is renamed aside by [`install`], and the system may then report the new
/// name. Call [`remember_exe`] early to fix it.
pub fn exe() -> Option<&'static Path> {
    remember_exe();
    EXE.get().and_then(|exe| exe.as_deref())
}

static EXE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Notes the executable's path. Call it at startup, before anything is installed.
pub fn remember_exe() {
    EXE.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        // Through a symlink, the update replaces the file it points to. Not on
        // Windows, where a canonical path starts with `\\?\`.
        #[cfg(unix)]
        let exe = fs::canonicalize(&exe).unwrap_or(exe);
        Some(exe)
    });
}

/// Where an update is downloaded: next to the executable, so it can be renamed
/// into place.
pub fn staging_path(exe: &Path) -> PathBuf {
    with_suffix(exe, ".update")
}

/// Where Windows' running executable is moved by [`install`].
pub fn old_path(exe: &Path) -> PathBuf {
    with_suffix(exe, ".old")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Replaces `exe` with `staged`. The running process keeps its build; the next
/// start runs the new one. If the swap fails part way, `exe` is put back.
pub fn install(staged: &Path, exe: &Path) -> Result<(), UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(staged, fs::Permissions::from_mode(0o755))
            .map_err(|err| UpdateError::io(staged, err))?;
        fs::rename(staged, exe).map_err(|err| UpdateError::io(exe, err))
    }
    #[cfg(not(unix))]
    {
        let old = old_path(exe);
        // Left by an update a restart hasn't cleaned up; gone unless that build runs.
        if old.exists()
            && let Err(err) = crate::fsx::remove_file(&old)
        {
            // It is this process: a second update before a restart. Then `exe` is
            // the first update's build, which nothing runs, so it can just be replaced.
            return crate::fsx::rename(staged, exe).map_err(|_| UpdateError::io(&old, err));
        }
        crate::fsx::rename(exe, &old).map_err(|err| UpdateError::io(exe, err))?;
        if let Err(err) = crate::fsx::rename(staged, exe) {
            let _ = crate::fsx::rename(&old, exe);
            return Err(UpdateError::io(exe, err));
        }
        Ok(())
    }
}

/// Removes what an earlier update left next to `exe`: Windows' old executable, and a
/// download that didn't finish.
pub fn clean_up(exe: &Path) {
    for leftover in [
        old_path(exe),
        staging_path(exe),
        with_suffix(&staging_path(exe), ".part"),
    ] {
        if leftover.exists() {
            let _ = fs::remove_file(&leftover);
        }
    }
}

/// The arguments for the restarted launcher: this one's, without an earlier
/// [`AFTER_UPDATE_ARG`], and with this process's id after it.
pub fn restart_args(args: impl IntoIterator<Item = OsString>, pid: u32) -> Vec<OsString> {
    let mut out = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == AFTER_UPDATE_ARG {
            args.next();
            continue;
        }
        out.push(arg);
    }
    out.push(AFTER_UPDATE_ARG.into());
    out.push(pid.to_string().into());
    out
}

/// Starts `exe` with this launcher's arguments. The new process waits for this one
/// to exit ([`wait_for_previous`]); quit right after.
pub fn restart(exe: &Path) -> io::Result<()> {
    let args = restart_args(std::env::args_os().skip(1), std::process::id());
    std::process::Command::new(exe).args(args).spawn().map(drop)
}

/// The process id after [`AFTER_UPDATE_ARG`], if the launcher was restarted.
pub fn previous_pid(args: impl IntoIterator<Item = OsString>) -> Option<u32> {
    let mut args = args.into_iter();
    args.find(|arg| arg == AFTER_UPDATE_ARG)?;
    args.next()?.to_str()?.parse().ok()
}

/// If this launcher was started by [`restart`], waits (a few seconds at most) for
/// the old one to exit, so the two don't both have the data folder open.
pub fn wait_for_previous() {
    if let Some(pid) = previous_pid(std::env::args_os().skip(1)) {
        wait_for_exit(pid, WAIT_FOR_PREVIOUS);
    }
}

#[cfg(windows)]
fn wait_for_exit(pid: u32, timeout: Duration) {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    // SAFETY: OpenProcess returns a handle or null; the handle is closed once waited on.
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return; // already gone
        }
        WaitForSingleObject(handle, timeout.as_millis().min(u32::MAX as u128) as u32);
        CloseHandle(handle);
    }
}

#[cfg(unix)]
fn wait_for_exit(pid: u32, timeout: Duration) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    let start = std::time::Instant::now();
    // SAFETY: signal 0 only checks that the process exists.
    while unsafe { libc::kill(pid, 0) } == 0 && start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(any(unix, windows)))]
fn wait_for_exit(_pid: u32, _timeout: Duration) {}

/// The Windows installer's AppId (launcher/installer/windows/lodestone-launcher.iss).
pub const WINDOWS_APP_ID: &str = "{19BD6256-FCCA-4D23-B640-06275F57BD67}";

/// Lodestone Host's Windows installer's AppId (launcher/installer/windows/lodestone-host.iss).
pub const HOST_WINDOWS_APP_ID: &str = "{6F0B2C55-4E0A-4D6B-9C8E-3B1A7D2E5F40}";

/// How Settings > Apps shows a build. The installer writes it the same way.
pub fn display_version(version: &str, build: u64) -> String {
    format!("{version} (build {build})")
}

/// Whether the installer's `InstallLocation` is the folder `exe` is in.
#[cfg(windows)]
fn is_install_location(install_location: &str, exe: &Path) -> bool {
    let Some(dir) = exe.parent() else {
        return false;
    };
    let normal = |path: &str| path.trim_end_matches(['\\', '/']).to_lowercase();
    !install_location.trim().is_empty()
        && normal(install_location) == normal(&dir.to_string_lossy())
}

/// Keeps the installer's entry in Settings > Apps showing the running build: after
/// the launcher updates itself it would still show the version the installer put
/// there. Only changes the entry if it points at the folder `exe` is in, so a
/// launcher run from elsewhere leaves an installed one alone. Returns whether it
/// changed anything.
pub fn refresh_uninstall_entry(exe: &Path, display: &str) -> bool {
    refresh_uninstall_entry_for(WINDOWS_APP_ID, exe, display)
}

/// [`refresh_uninstall_entry`] for the installer with AppId `app_id`.
#[cfg(windows)]
pub fn refresh_uninstall_entry_for(app_id: &str, exe: &Path, display: &str) -> bool {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
            RegOpenKeyExW, RegSetValueExW,
        },
    };

    let subkey = wide(&format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}_is1"
    ));
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: the strings are NUL-terminated UTF-16, and the key is closed below.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        )
    };
    if opened != ERROR_SUCCESS {
        return false; // not installed with the installer
    }
    let changed = (|| {
        let location = read_registry_string(key, "InstallLocation")?;
        if !is_install_location(&location, exe) {
            return Some(false);
        }
        if read_registry_string(key, "DisplayVersion").as_deref() == Some(display) {
            return Some(false);
        }
        let name = wide("DisplayVersion");
        let value = wide(display);
        // SAFETY: `value` is NUL-terminated UTF-16 and its size in bytes includes the NUL.
        let set = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        };
        Some(set == ERROR_SUCCESS)
    })();
    // SAFETY: `key` was opened above.
    unsafe { RegCloseKey(key) };
    changed.unwrap_or(false)
}

#[cfg(not(windows))]
pub fn refresh_uninstall_entry_for(_app_id: &str, _exe: &Path, _display: &str) -> bool {
    false
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn read_registry_string(
    key: windows_sys::Win32::System::Registry::HKEY,
    name: &str,
) -> Option<String> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{RRF_RT_REG_SZ, RegGetValueW},
    };

    let name = wide(name);
    let mut bytes = 0u32;
    // SAFETY: a size query; nothing is written but `bytes`.
    let sized = unsafe {
        RegGetValueW(
            key,
            std::ptr::null(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if sized != ERROR_SUCCESS {
        return None;
    }
    let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
    // SAFETY: `buffer` holds at least `bytes` bytes.
    let read = unsafe {
        RegGetValueW(
            key,
            std::ptr::null(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    Some(
        String::from_utf16_lossy(&buffer)
            .trim_end_matches('\0')
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(build: u64) -> Manifest {
        let mut assets = BTreeMap::new();
        assets.insert(
            platform(),
            Asset {
                name: format!("lodestone-launcher-{build}"),
                size: 3,
                sha256: format!("{:x}", Sha256::digest(b"new")),
                build: None,
                commit: None,
                core: None,
            },
        );
        Manifest {
            build,
            commit: "0123456789abcdef".into(),
            version: "0.1.0".into(),
            published: None,
            tag: None,
            assets,
        }
    }

    const DOWNLOAD: &str = "https://github.com/Lodestone-Team/lodestone/releases/download";

    /// A release as GitHub's API lists it, with a build for each of `builds`.
    fn release(tag: &str, prerelease: bool, builds: &[(&str, u64)]) -> Release {
        let mut assets: Vec<ReleaseAsset> = builds
            .iter()
            .flat_map(|(platform, build)| {
                let exe = if platform.starts_with("windows-") {
                    ".exe"
                } else {
                    ""
                };
                [
                    format!("lodestone-launcher-{build}-{platform}-standalone{exe}"),
                    format!("lodestone-launcher-{build}-{platform}-setup.exe"),
                ]
            })
            .chain([
                MANIFEST_NAME.to_string(),
                "LodestoneLauncherSetup.exe".into(),
            ])
            .map(|name| ReleaseAsset {
                browser_download_url: format!("{DOWNLOAD}/{tag}/{name}"),
                name,
            })
            .collect();
        assets.reverse();
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease,
            assets,
        }
    }

    fn picked(releases: &[Release], channel: UpdateChannel) -> Option<(String, u64)> {
        pick_release(releases, channel, "windows-x86_64").map(|c| (c.tag, c.build))
    }

    #[test]
    fn stable_launchers_take_the_newest_stable_release() {
        let releases = [
            release("launcher-v0.3.0-beta.1", true, &[("windows-x86_64", 40)]),
            release("launcher-v0.2.1", false, &[("windows-x86_64", 31)]),
            release("launcher-v0.2.0", false, &[("windows-x86_64", 30)]),
        ];
        let found = pick_release(&releases, UpdateChannel::Stable, "windows-x86_64").unwrap();
        assert_eq!((found.tag.as_str(), found.build), ("launcher-v0.2.1", 31));
        assert_eq!(
            found.manifest_url,
            format!("{DOWNLOAD}/launcher-v0.2.1/latest.json")
        );
    }

    #[test]
    fn beta_launchers_take_whichever_is_newer() {
        let beta = release("launcher-v0.3.0-beta.1", true, &[("windows-x86_64", 40)]);
        let stable = release("launcher-v0.2.1", false, &[("windows-x86_64", 31)]);
        assert_eq!(
            picked(&[beta.clone(), stable.clone()], UpdateChannel::Beta),
            Some(("launcher-v0.3.0-beta.1".into(), 40))
        );
        // A stable hotfix made after the beta, from a newer commit.
        let hotfix = release("launcher-v0.2.2", false, &[("windows-x86_64", 45)]);
        assert_eq!(
            picked(&[hotfix, beta, stable], UpdateChannel::Beta),
            Some(("launcher-v0.2.2".into(), 45))
        );
    }

    #[test]
    fn the_build_orders_releases_not_the_list() {
        // A stable version cut from an older commit after a newer one was published.
        let releases = [
            release("launcher-v0.2.1", false, &[("windows-x86_64", 20)]),
            release("launcher-v0.3.0", false, &[("windows-x86_64", 30)]),
        ];
        assert_eq!(
            picked(&releases, UpdateChannel::Stable),
            Some(("launcher-v0.3.0".into(), 30))
        );
    }

    #[test]
    fn drafts_other_products_and_other_platforms_are_ignored() {
        let mut draft = release("launcher-v9.0.0", false, &[("windows-x86_64", 900)]);
        draft.draft = true;
        let host = release("v9.0.0", false, &[("windows-x86_64", 800)]);
        let mac_only = release("launcher-v0.5.0", false, &[("macos-aarch64", 70)]);
        let mut no_manifest = release("launcher-v0.4.0", false, &[("windows-x86_64", 60)]);
        no_manifest.assets.retain(|a| a.name != MANIFEST_NAME);
        let ok = release("launcher-v0.3.0", false, &[("windows-x86_64", 30)]);
        assert_eq!(
            picked(
                &[draft, host, mac_only.clone(), no_manifest, ok],
                UpdateChannel::Beta
            ),
            Some(("launcher-v0.3.0".into(), 30))
        );
        assert_eq!(picked(&[mac_only], UpdateChannel::Stable), None);
        assert_eq!(picked(&[], UpdateChannel::Stable), None);
    }

    #[test]
    fn reads_the_build_from_the_standalone_binarys_name() {
        let windows = "windows-x86_64";
        assert_eq!(
            asset_build(
                "lodestone-launcher-3480-windows-x86_64-standalone.exe",
                windows
            ),
            Some(3480)
        );
        assert_eq!(
            asset_build(
                "lodestone-launcher-12-macos-aarch64-standalone",
                "macos-aarch64"
            ),
            Some(12)
        );
        for other in [
            "lodestone-launcher-3480-windows-x86_64-setup.exe",
            "lodestone-launcher-3480-windows-aarch64-standalone.exe",
            "lodestone-launcher-3480-windows-x86_64-standalone.exe.sig",
            "lodestone-launcher-x-windows-x86_64-standalone.exe",
            "lodestone-launcher--windows-x86_64-standalone.exe",
            "LodestoneLauncherSetup.exe",
            "latest.json",
        ] {
            assert_eq!(asset_build(other, windows), None, "{other}");
        }
    }

    /// Looks up the real releases, as a launcher would. Run with
    /// `cargo test -p launcher-core --lib update::tests::live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_lookup_on_github() {
        let http = crate::http::default_client().unwrap();
        for channel in [UpdateChannel::Stable, UpdateChannel::Beta] {
            let found = futures::executor::block_on(crate::http::in_http_runtime(find_update(
                http.as_ref(),
                channel,
            )));
            match found {
                Ok(found) => println!(
                    "{channel:?}: {:?}",
                    found.map(|f| (f.build(), f.tag, f.url))
                ),
                Err(UpdateError::RateLimited) => println!("{channel:?}: rate limited"),
                Err(err) => panic!("{channel:?}: {err}"),
            }
        }
    }

    #[test]
    fn reads_githubs_release_list() {
        let body = serde_json::json!([{
            "tag_name": "launcher-v0.2.0",
            "draft": false,
            "prerelease": false,
            "name": "Lodestone Launcher 0.2.0",
            "body": "notes",
            "assets": [{
                "name": "latest.json",
                "browser_download_url": format!("{DOWNLOAD}/launcher-v0.2.0/latest.json"),
                "size": 460,
                "uploader": {"login": "someone"}
            }]
        }]);
        let releases: Vec<Release> = serde_json::from_value(body).unwrap();
        assert_eq!(releases[0].tag_name, "launcher-v0.2.0");
        assert_eq!(releases[0].assets[0].name, "latest.json");
    }

    #[test]
    fn reads_the_build_ci_compiled_in() {
        assert_eq!(Build::parse(None, Some("abc")), None);
        assert_eq!(Build::parse(Some("dev"), None), None);
        let build = Build::parse(Some(" 42 "), Some("0123456789abcdef")).unwrap();
        assert_eq!(build.number, 42);
        assert_eq!(build.short_commit(), Some("0123456"));
        assert_eq!(Build::parse(Some("7"), Some("")).unwrap().commit, None);
    }

    #[test]
    fn offers_only_a_newer_build_for_this_platform() {
        let latest = manifest(10);
        assert!(latest.newer(9).is_some());
        assert!(latest.newer(10).is_none());
        assert!(latest.newer(11).is_none());
        let mut elsewhere = latest.clone();
        elsewhere.assets = BTreeMap::new();
        assert!(elsewhere.newer(1).is_none());
    }

    #[test]
    fn each_platform_has_its_own_build() {
        // Another platform was published later, at build 20; this one is still 10.
        let mut latest = manifest(20);
        latest.assets.get_mut(&platform()).unwrap().build = Some(10);
        assert_eq!(latest.for_platform().unwrap().0, 10);
        assert!(latest.newer(10).is_none());
        assert_eq!(latest.newer(9).unwrap().0, 10);
    }

    #[test]
    fn files_are_next_to_the_manifest() {
        let asset = &manifest(5).assets[&platform()];
        assert_eq!(
            Manifest::asset_url(&format!("{DOWNLOAD}/launcher-v0.2.0/latest.json"), asset).unwrap(),
            format!("{DOWNLOAD}/launcher-v0.2.0/lodestone-launcher-5")
        );
        assert_eq!(
            Manifest::asset_url("http://127.0.0.1:8000/latest.json", asset).unwrap(),
            "http://127.0.0.1:8000/lodestone-launcher-5"
        );
        assert!(Manifest::asset_url("http://example.com/latest.json", asset).is_err());
    }

    #[test]
    fn only_https_or_this_machine() {
        assert!(check_url("https://github.com/x").is_ok());
        assert!(check_url("http://localhost:1/x").is_ok());
        assert!(check_url("http://[::1]:1/x").is_ok());
        assert!(check_url("http://github.com/x").is_err());
        assert!(check_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn refuses_manifests_that_point_elsewhere() {
        let mut bad = manifest(3);
        bad.assets.get_mut(&platform()).unwrap().name = "../../evil".into();
        let body = serde_json::to_vec(&bad).unwrap();
        assert!(parse_manifest(&body).is_err());

        let mut bad = manifest(3);
        bad.assets.get_mut(&platform()).unwrap().sha256 = "zz".into();
        assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());

        let good = manifest(3);
        assert_eq!(
            parse_manifest(&serde_json::to_vec(&good).unwrap()).unwrap(),
            good
        );
        // Fields a later CI adds are ignored.
        let mut extra = serde_json::to_value(&good).unwrap();
        extra["channel"] = "nightly".into();
        assert!(parse_manifest(&serde_json::to_vec(&extra).unwrap()).is_ok());
    }

    #[test]
    fn host_manifests_list_the_core_too() {
        let mut host = manifest(7);
        let core = Companion {
            name: "lodestone-host-7-macos-aarch64-core".into(),
            size: 5,
            sha256: format!("{:x}", Sha256::digest(b"core!")),
        };
        host.assets.get_mut(&platform()).unwrap().core = Some(core.clone());
        let body = serde_json::to_vec(&host).unwrap();
        let read = parse_manifest(&body).unwrap();
        assert_eq!(read.for_platform().unwrap().1.core.as_ref(), Some(&core));
        assert_eq!(core.as_asset().sha256, core.sha256);

        let mut bad = host.clone();
        bad.assets.get_mut(&platform()).unwrap().core = Some(Companion {
            name: "../lodestone_core".into(),
            ..core
        });
        assert!(parse_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
        // A launcher's manifest has no core, and doesn't write one.
        let launcher = serde_json::to_value(manifest(7)).unwrap();
        assert!(launcher["assets"][platform()].get("core").is_none());
    }

    #[test]
    fn the_host_reads_its_own_releases() {
        let platform = "macos-aarch64";
        let host_release = |tag: &str, prerelease: bool, build: u64| Release {
            tag_name: tag.into(),
            draft: false,
            prerelease,
            assets: [
                format!("lodestone-host-{build}-{platform}-standalone"),
                format!("lodestone-host-{build}-{platform}-core"),
                format!("lodestone-host-{build}-macos-universal.dmg"),
                MANIFEST_NAME.into(),
            ]
            .into_iter()
            .map(|name| ReleaseAsset {
                browser_download_url: format!("{DOWNLOAD}/{tag}/{name}"),
                name,
            })
            .collect(),
        };
        let releases = [
            release("launcher-v9.0.0", false, &[(platform, 900)]),
            host_release("host-v0.7.0-beta.1", true, 60),
            host_release("host-v0.6.0", false, 50),
            // the Tauri Host's and the core's own releases
            release("v0.5.1", false, &[(platform, 800)]),
        ];
        let stable =
            pick_release_for(Product::HOST, &releases, UpdateChannel::Stable, platform).unwrap();
        assert_eq!((stable.tag.as_str(), stable.build), ("host-v0.6.0", 50));
        let beta =
            pick_release_for(Product::HOST, &releases, UpdateChannel::Beta, platform).unwrap();
        assert_eq!((beta.tag.as_str(), beta.build), ("host-v0.7.0-beta.1", 60));
        // and launchers never take the Host's
        assert_eq!(
            pick_release(&releases, UpdateChannel::Beta, platform).map(|c| c.tag),
            Some("launcher-v9.0.0".into())
        );
        assert_eq!(
            Product::HOST.asset_build(
                "lodestone-host-12-windows-x86_64-standalone.exe",
                "windows-x86_64"
            ),
            Some(12)
        );
        assert_eq!(
            Product::HOST.asset_build(
                "lodestone-host-12-windows-x86_64-core.exe",
                "windows-x86_64"
            ),
            None
        );
    }

    #[test]
    fn restart_passes_the_arguments_on() {
        let args = ["--scene", "x", AFTER_UPDATE_ARG, "12", "-v"].map(OsString::from);
        let out = restart_args(args, 99);
        assert_eq!(
            out,
            ["--scene", "x", "-v", AFTER_UPDATE_ARG, "99"].map(OsString::from)
        );
        assert_eq!(previous_pid(out), Some(99));
        assert_eq!(previous_pid([OsString::from("-v")]), None);
    }

    #[test]
    fn installs_over_the_executable_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(if cfg!(windows) {
            "lodestone-launcher.exe"
        } else {
            "lodestone-launcher"
        });
        fs::write(&exe, b"old").unwrap();
        let staged = staging_path(&exe);
        fs::write(&staged, b"new").unwrap();
        install(&staged, &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert!(!staged.exists());
        if cfg!(windows) {
            assert_eq!(fs::read(old_path(&exe)).unwrap(), b"old");
        }
        // A second update while the first's old file is still there.
        fs::write(&staged, b"newer").unwrap();
        install(&staged, &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"newer");

        clean_up(&exe);
        assert!(!old_path(&exe).exists());
        assert_eq!(fs::read(&exe).unwrap(), b"newer");
    }

    #[test]
    fn a_missing_staged_file_leaves_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("launcher");
        fs::write(&exe, b"old").unwrap();
        assert!(install(&staging_path(&exe), &exe).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"old");
    }

    #[test]
    fn apps_entry_shows_the_build() {
        assert_eq!(display_version("0.1.0", 531), "0.1.0 (build 531)");
    }

    // Windows paths: `Path` only splits them on Windows.
    #[cfg(windows)]
    #[test]
    fn the_install_location_is_the_executables_folder() {
        let exe = Path::new(
            r"C:\Users\me\AppData\Local\Programs\Lodestone Launcher\lodestone-launcher.exe",
        );
        // Inno Setup writes the folder with a trailing backslash.
        assert!(is_install_location(
            r"C:\Users\me\AppData\Local\Programs\Lodestone Launcher\",
            exe
        ));
        assert!(is_install_location(
            r"c:\users\me\appdata\local\programs\lodestone launcher",
            exe
        ));
        assert!(!is_install_location(r"D:\Games\Lodestone Launcher\", exe));
        assert!(!is_install_location("", exe));
    }

    #[test]
    fn a_launcher_that_wasnt_installed_leaves_the_apps_entry_alone() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("lodestone-launcher.exe");
        assert!(!refresh_uninstall_entry(&exe, "0.1.0 (build 1)"));
    }

    #[test]
    fn waiting_for_a_process_that_is_gone_returns() {
        // Not a real process id on any platform the tests run on.
        let start = std::time::Instant::now();
        wait_for_exit(u32::MAX - 1, Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_secs(4));
    }
}
