//! Updating the launcher from its GitHub builds.
//!
//! A developer publishes a release with `launcher/scripts/publish_release.py`: it
//! builds the launcher for their platform and uploads it to the `launcher-latest`
//! GitHub release, then updates the release's `latest.json` [`Manifest`]: each
//! platform's file with its build number, commit, size and SHA-256. Platforms are
//! published separately (each from a machine of its own), so each has its own build.
//!
//! A build's number is the count of commits in its history (`git rev-list --count`),
//! which grows along `main`. The script compiles it in as `LODESTONE_LAUNCHER_BUILD`;
//! a plain `cargo build` has none, and never updates itself.
//!
//! An update is downloaded next to the running executable, checked against the
//! manifest, and swapped in for it ([`install`]): on Unix by renaming over it, on
//! Windows by first renaming the running executable aside (Windows allows that, but
//! not replacing it), to `<exe>.old`, which the next start removes ([`clean_up`]).
//! The process keeps running the old build until it restarts ([`restart`]); a plain
//! quit and start also runs the new one.

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

/// Where releases are published.
pub const MANIFEST_URL: &str =
    "https://github.com/Lodestone-Team/lodestone/releases/download/launcher-latest/latest.json";

/// Overrides [`MANIFEST_URL`], to test updating against a local server.
pub const MANIFEST_URL_ENV: &str = "LODESTONE_LAUNCHER_UPDATE_URL";

/// The argument a restarted launcher gets: the process id of the launcher that
/// started it, which it waits to exit before it opens its window.
pub const AFTER_UPDATE_ARG: &str = "--after-update";

/// The manifest is a few hundred bytes; anything far larger is not one.
const MAX_MANIFEST_BYTES: usize = 256 * 1024;

/// A launcher binary is tens of MB. A larger file is refused before it is read.
const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;

/// How long a restarted launcher waits for the old one to exit.
const WAIT_FOR_PREVIOUS: Duration = Duration::from_secs(15);

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

    fn parse(number: Option<&str>, commit: Option<&'static str>) -> Option<Build> {
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

/// The manifest URL in use: [`MANIFEST_URL`], or the one in [`MANIFEST_URL_ENV`].
pub fn manifest_url() -> String {
    std::env::var(MANIFEST_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| MANIFEST_URL.to_string())
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
        // Only a plain file name, so it can't point outside the release.
        let plain = !asset.name.is_empty()
            && !asset.name.contains(['/', '\\', '?', '#'])
            && asset.name != "."
            && asset.name != "..";
        if !plain {
            return Err(UpdateError::Manifest(format!(
                "bad file name {:?}",
                asset.name
            )));
        }
        if asset.sha256.len() != 64 || !asset.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(UpdateError::Manifest(format!(
                "bad SHA-256 for {}",
                asset.name
            )));
        }
        if asset.size == 0 || asset.size > MAX_BINARY_BYTES {
            return Err(UpdateError::Manifest(format!(
                "bad size for {}",
                asset.name
            )));
        }
    }
    Ok(manifest)
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
        Ok(()) => fs::rename(&part, dest).map_err(|err| {
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
        if old.exists() {
            fs::remove_file(&old).map_err(|err| UpdateError::io(&old, err))?;
        }
        fs::rename(exe, &old).map_err(|err| UpdateError::io(exe, err))?;
        if let Err(err) = fs::rename(staged, exe) {
            let _ = fs::rename(&old, exe);
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
#[cfg(windows)]
pub fn refresh_uninstall_entry(exe: &Path, display: &str) -> bool {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
            RegOpenKeyExW, RegSetValueExW,
        },
    };

    let subkey = wide(&format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{WINDOWS_APP_ID}_is1"
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
pub fn refresh_uninstall_entry(_exe: &Path, _display: &str) -> bool {
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
            },
        );
        Manifest {
            build,
            commit: "0123456789abcdef".into(),
            version: "0.1.0".into(),
            published: None,
            assets,
        }
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
            Manifest::asset_url(MANIFEST_URL, asset).unwrap(),
            "https://github.com/Lodestone-Team/lodestone/releases/download/launcher-latest/lodestone-launcher-5"
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
