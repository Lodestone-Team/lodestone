//! The download engine: many files in parallel, up to a limit, each checked against
//! its SHA-1, retried with backoff, and skipped when a file with the right hash is
//! already there. It reports progress in files and bytes and can be cancelled.
//!
//! A file is written to a temporary name next to its destination and renamed into
//! place only once its hash matches, so a cancelled or failed download never leaves a
//! partial file where the game would read it.

use std::{
    fs::{self, File},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::{AsyncReadExt as _, TryStreamExt as _, future::BoxFuture};
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use sha1::{Digest as _, Sha1};

use super::record::{self, InstallRecord};
use crate::USER_AGENT;

/// The number of files downloaded at once.
pub const DEFAULT_CONCURRENCY: usize = 8;
/// Attempts per file, including the first.
pub const DEFAULT_ATTEMPTS: u32 = 4;
/// The wait before the first retry; it doubles each time.
pub const DEFAULT_BACKOFF: Duration = Duration::from_millis(500);

/// A file to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadItem {
    /// The URL to fetch, already checked by [`MojangUrls`](super::mojang::MojangUrls).
    pub url: String,
    pub dest: PathBuf,
    /// The expected SHA-1, as hex.
    pub sha1: String,
    /// The expected size, when known. It is used for progress and checked.
    pub size: Option<u64>,
    /// Set the executable bits (for the Java runtime's binaries).
    pub executable: bool,
}

/// Progress counters, shared between the engine and whoever shows them.
#[derive(Debug, Default)]
pub struct Progress {
    files_total: AtomicU64,
    files_done: AtomicU64,
    bytes_total: AtomicU64,
    bytes_done: AtomicU64,
}

/// A snapshot of [`Progress`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub files_total: u64,
    pub files_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
}

impl Progress {
    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files_total: self.files_total.load(Ordering::Relaxed),
            files_done: self.files_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
        }
    }

    /// Sets every counter, for previews of the progress.
    pub fn set(&self, snapshot: ProgressSnapshot) {
        self.files_total
            .store(snapshot.files_total, Ordering::Relaxed);
        self.files_done
            .store(snapshot.files_done, Ordering::Relaxed);
        self.bytes_total
            .store(snapshot.bytes_total, Ordering::Relaxed);
        self.bytes_done
            .store(snapshot.bytes_done, Ordering::Relaxed);
    }

    fn reset(&self, files: u64, bytes: u64) {
        self.files_total.store(files, Ordering::Relaxed);
        self.files_done.store(0, Ordering::Relaxed);
        self.bytes_total.store(bytes, Ordering::Relaxed);
        self.bytes_done.store(0, Ordering::Relaxed);
    }

    fn add_bytes(&self, bytes: u64) {
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Takes back bytes counted for an attempt that failed.
    fn sub_bytes(&self, bytes: u64) {
        let mut done = self.bytes_done.load(Ordering::Relaxed);
        while let Err(actual) = self.bytes_done.compare_exchange_weak(
            done,
            done.saturating_sub(bytes),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            done = actual;
        }
    }

    fn file_done(&self) {
        self.files_done.fetch_add(1, Ordering::Relaxed);
    }
}

/// Cancels a download. Cloning shares the flag.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Waits for a duration. The app passes GPUI's timer; tests pass one that records the
/// wait and returns at once.
pub type Sleep = Arc<dyn Fn(Duration) -> BoxFuture<'static, ()> + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("the download was cancelled")]
    Cancelled,
    #[error("could not download {url}: {reason}")]
    Failed { url: String, reason: String },
    #[error("could not write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Why one attempt failed, and whether trying again might help.
#[derive(Debug)]
struct AttemptError {
    reason: String,
    retry: bool,
}

/// The downloader. Cheap to clone.
#[derive(Clone)]
pub struct Downloader {
    http: Arc<dyn HttpClient>,
    concurrency: usize,
    attempts: u32,
    backoff: Duration,
    sleep: Sleep,
}

impl Downloader {
    pub fn new(http: Arc<dyn HttpClient>, sleep: Sleep) -> Self {
        Self {
            http,
            concurrency: DEFAULT_CONCURRENCY,
            attempts: DEFAULT_ATTEMPTS,
            backoff: DEFAULT_BACKOFF,
            sleep,
        }
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts.max(1);
        self
    }

    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    /// Downloads `items`, resetting `progress` to their totals. Every file already
    /// present is hashed (in parallel) and kept if it matches. Stops at the first file
    /// that fails after its retries, or when `cancel` is set.
    pub async fn run(
        &self,
        items: Vec<DownloadItem>,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<(), DownloadError> {
        let mut record = InstallRecord::default();
        self.run_recorded(
            items,
            Path::new("/"),
            &mut record,
            Verify::Full,
            progress,
            cancel,
            &RunStats::default(),
        )
        .await
    }

    /// Like [`Downloader::run`], with an install record of the files under `root`.
    ///
    /// With [`Verify::Recorded`], a file whose size and modification time match its
    /// record is trusted without being read; the others are hashed. With
    /// [`Verify::Full`], every file present is hashed. Either way `record` is
    /// rebuilt to hold exactly the files that were trusted, verified or downloaded,
    /// also when an error stops the run part way, so the caller can save it.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_recorded(
        &self,
        items: Vec<DownloadItem>,
        root: &Path,
        record: &mut InstallRecord,
        verify: Verify,
        progress: &Progress,
        cancel: &CancelToken,
        stats: &RunStats,
    ) -> Result<(), DownloadError> {
        let items = dedupe(items);
        progress.reset(
            items.len() as u64,
            items.iter().filter_map(|item| item.size).sum(),
        );
        let keys: Vec<Option<String>> = items
            .iter()
            .map(|item| record::relative_key(root, &item.dest))
            .collect();

        // Check what is already there, in parallel: a stat for each file, and a hash
        // for those the record does not vouch for.
        let previous = std::mem::replace(record, InstallRecord::new(record.key.clone()));
        let checks = parallel_map(&items, |index, item| {
            if cancel.is_cancelled() {
                return Check::Missing;
            }
            let now = record::stat(&item.dest);
            if verify == Verify::Recorded
                && let Some(key) = &keys[index]
                && previous.trusts(key, &item.sha1, now)
            {
                stats.trusted.fetch_add(1, Ordering::Relaxed);
                return Check::Good(now.expect("trusted files exist"));
            }
            let Some(now) = now else {
                return Check::Missing;
            };
            stats.hashed.fetch_add(1, Ordering::Relaxed);
            if file_matches(&item.dest, &item.sha1) {
                Check::Good(now)
            } else {
                Check::Missing
            }
        });
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        let mut missing = Vec::new();
        for ((item, key), check) in items.iter().zip(&keys).zip(checks) {
            match check {
                Check::Good(stat) => {
                    set_executable(item).map_err(|source| DownloadError::Io {
                        path: item.dest.clone(),
                        source,
                    })?;
                    if let Some(key) = key {
                        record.insert(key.clone(), &item.sha1, stat);
                    }
                    progress.add_bytes(item.size.unwrap_or(0));
                    progress.file_done();
                }
                Check::Missing => missing.push((item, key)),
            }
        }

        // Download the rest.
        let downloaded = Mutex::new(Vec::new());
        let result = futures::stream::iter(missing.into_iter().map(Ok))
            .try_for_each_concurrent(self.concurrency, |(item, key)| {
                let downloaded = &downloaded;
                async move {
                    self.download(item, progress, cancel).await?;
                    stats.downloaded.fetch_add(1, Ordering::Relaxed);
                    if let (Some(key), Some(stat)) = (key, record::stat(&item.dest)) {
                        downloaded.lock().unwrap_or_else(|p| p.into_inner()).push((
                            key.clone(),
                            item.sha1.clone(),
                            stat,
                        ));
                    }
                    Ok(())
                }
            })
            .await;
        for (key, sha1, stat) in downloaded.into_inner().unwrap_or_else(|p| p.into_inner()) {
            record.insert(key, &sha1, stat);
        }
        result
    }

    /// Downloads one file, unless it is already there with the right hash.
    pub async fn fetch(
        &self,
        item: &DownloadItem,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<(), DownloadError> {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        if file_matches(&item.dest, &item.sha1) {
            set_executable(item).map_err(|source| DownloadError::Io {
                path: item.dest.clone(),
                source,
            })?;
            progress.add_bytes(item.size.unwrap_or(0));
            progress.file_done();
            return Ok(());
        }
        self.download(item, progress, cancel).await
    }

    /// Downloads one file, with retries, whatever is there now.
    async fn download(
        &self,
        item: &DownloadItem,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<(), DownloadError> {
        let mut last_error = String::new();
        for attempt in 0..self.attempts {
            if attempt > 0 {
                (self.sleep)(self.backoff * 2u32.saturating_pow(attempt - 1)).await;
            }
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            let mut counted = 0;
            let result =
                crate::http::in_http_runtime(self.attempt(item, progress, cancel, &mut counted))
                    .await;
            match result {
                Ok(()) => {
                    // Count the expected size, so the total adds up even when a
                    // server sends a different Content-Length.
                    progress.sub_bytes(counted);
                    progress.add_bytes(item.size.unwrap_or(counted));
                    progress.file_done();
                    return Ok(());
                }
                Err(Attempt::Cancelled) => {
                    progress.sub_bytes(counted);
                    return Err(DownloadError::Cancelled);
                }
                Err(Attempt::Io(source)) => {
                    progress.sub_bytes(counted);
                    return Err(DownloadError::Io {
                        path: item.dest.clone(),
                        source,
                    });
                }
                Err(Attempt::Failed(error)) => {
                    progress.sub_bytes(counted);
                    last_error = error.reason;
                    if !error.retry {
                        break;
                    }
                }
            }
        }
        Err(DownloadError::Failed {
            url: item.url.clone(),
            reason: last_error,
        })
    }

    /// The SHA-1 of the file at `url`, read through without saving it, with retries
    /// as for a download. For a jar whose SHA-1 isn't published anywhere: unlike
    /// [`crate::http::send`], this has no size limit, since such jars (old Forge's
    /// Scala libraries) can be megabytes.
    pub async fn sha1_of_url(
        &self,
        url: &str,
        cancel: &CancelToken,
    ) -> Result<String, DownloadError> {
        let mut last_error = String::new();
        for attempt in 0..self.attempts {
            if attempt > 0 {
                (self.sleep)(self.backoff * 2u32.saturating_pow(attempt - 1)).await;
            }
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            match crate::http::in_http_runtime(self.hash_attempt(url, cancel)).await {
                Ok(sha1) => return Ok(sha1),
                Err(Attempt::Cancelled) => return Err(DownloadError::Cancelled),
                Err(Attempt::Io(source)) => {
                    return Err(DownloadError::Failed {
                        url: url.to_string(),
                        reason: source.to_string(),
                    });
                }
                Err(Attempt::Failed(error)) => {
                    last_error = error.reason;
                    if !error.retry {
                        break;
                    }
                }
            }
        }
        Err(DownloadError::Failed {
            url: url.to_string(),
            reason: last_error,
        })
    }

    async fn hash_attempt(&self, url: &str, cancel: &CancelToken) -> Result<String, Attempt> {
        let response = self.get(url).await?;
        let mut body = response.into_body();
        let mut hasher = Sha1::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            if cancel.is_cancelled() {
                return Err(Attempt::Cancelled);
            }
            let read = body
                .read(&mut buffer)
                .await
                .map_err(|err| Attempt::failed(err.to_string(), true))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Sends a GET for `url`, following redirects, and checks the status.
    async fn get(&self, url: &str) -> Result<http_client::Response<AsyncBody>, Attempt> {
        let request = Request::builder()
            .method(Method::GET)
            .uri(url)
            .header("User-Agent", USER_AGENT)
            .follow_redirects(RedirectPolicy::FollowAll)
            .body(AsyncBody::empty())
            .map_err(|err| Attempt::failed(err.to_string(), false))?;
        let response = self
            .http
            .send(request)
            .await
            .map_err(|err| Attempt::failed(format!("{err:#}"), true))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            // Retry server errors and rate limiting; a 404 will not go away.
            let retry = status >= 500 || status == 408 || status == 429;
            return Err(Attempt::failed(format!("HTTP {status}"), retry));
        }
        Ok(response)
    }

    async fn attempt(
        &self,
        item: &DownloadItem,
        progress: &Progress,
        cancel: &CancelToken,
        counted: &mut u64,
    ) -> Result<(), Attempt> {
        let response = self.get(&item.url).await?;

        let dir = item
            .dest
            .parent()
            .ok_or_else(|| Attempt::Io(io::Error::other("the destination has no folder")))?;
        fs::create_dir_all(dir).map_err(Attempt::Io)?;
        let (tmp_path, mut file) = temp_file(&item.dest).map_err(Attempt::Io)?;
        let _cleanup = RemoveOnDrop(tmp_path.clone());

        let mut body = response.into_body();
        let mut hasher = Sha1::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(Attempt::Cancelled);
            }
            let read = body
                .read(&mut buffer)
                .await
                .map_err(|err| Attempt::failed(err.to_string(), true))?;
            if read == 0 {
                break;
            }
            let chunk = &buffer[..read];
            hasher.update(chunk);
            file.write_all(chunk).map_err(Attempt::Io)?;
            written += read as u64;
            *counted += read as u64;
            progress.add_bytes(read as u64);
            if let Some(size) = item.size
                && written > size
            {
                return Err(Attempt::failed(
                    format!("the server sent more than the expected {size} bytes"),
                    true,
                ));
            }
        }
        let hash = format!("{:x}", hasher.finalize());
        if !hash.eq_ignore_ascii_case(&item.sha1) {
            return Err(Attempt::failed(
                format!("the SHA-1 is {hash}, expected {}", item.sha1),
                true,
            ));
        }
        if let Some(size) = item.size
            && written != size
        {
            return Err(Attempt::failed(
                format!("got {written} bytes, expected {size}"),
                true,
            ));
        }
        // Flushing to disk blocks for milliseconds per file (tens on a slow disk), and
        // a fresh install has thousands of files. On a blocking thread, so the other
        // downloads in this future keep going meanwhile.
        let dest = item.dest.clone();
        reqwest_client::runtime()
            .spawn_blocking(move || {
                file.sync_all()?;
                drop(file);
                crate::fsx::rename(&tmp_path, &dest)
            })
            .await
            .map_err(|err| Attempt::Io(io::Error::other(err)))?
            .map_err(Attempt::Io)?;
        set_executable(item).map_err(Attempt::Io)?;
        Ok(())
    }
}

enum Attempt {
    Cancelled,
    Io(io::Error),
    Failed(AttemptError),
}

impl Attempt {
    fn failed(reason: String, retry: bool) -> Self {
        Attempt::Failed(AttemptError { reason, retry })
    }
}

/// How much to trust files that are already there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verify {
    /// Trust files whose size and modification time match the install record.
    #[default]
    Recorded,
    /// Hash every file ("Verify game files").
    Full,
}

/// What a run did with each file, for tracing and tests.
#[derive(Debug, Default)]
pub struct RunStats {
    /// Trusted from the install record without being read.
    pub trusted: AtomicU64,
    /// Read and hashed.
    pub hashed: AtomicU64,
    /// Downloaded.
    pub downloaded: AtomicU64,
}

impl RunStats {
    /// (trusted, hashed, downloaded).
    pub fn counts(&self) -> (u64, u64, u64) {
        (
            self.trusted.load(Ordering::Relaxed),
            self.hashed.load(Ordering::Relaxed),
            self.downloaded.load(Ordering::Relaxed),
        )
    }
}

enum Check {
    /// There with the right contents, as of this stat.
    Good(record::Stat),
    /// Missing, or with the wrong contents.
    Missing,
}

/// Maps `items` on a few threads. Stats and hashes are blocking calls; thousands of
/// asset objects go much faster spread over the cores.
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8);
    if items.len() < 64 || threads == 1 {
        return items
            .iter()
            .enumerate()
            .map(|(i, item)| f(i, item))
            .collect();
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .enumerate()
            .map(|(n, part)| {
                let f = &f;
                scope.spawn(move || {
                    part.iter()
                        .enumerate()
                        .map(|(i, item)| f(n * chunk + i, item))
                        .collect::<Vec<R>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a check thread panicked"))
            .collect()
    })
}

/// Removes a temporary file unless it was renamed into place.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temp_file(dest: &Path) -> io::Result<(PathBuf, File)> {
    let name = dest
        .file_name()
        .ok_or_else(|| io::Error::other("the destination has no file name"))?;
    for _ in 0..16 {
        let mut tmp = std::ffi::OsString::from(".");
        tmp.push(name);
        tmp.push(format!(
            ".{:016x}.part",
            u64::from_le_bytes(crate::random_bytes())
        ));
        let path = dest.with_file_name(tmp);
        match File::options().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::other("could not create a temporary file"))
}

/// Whether the file at `path` exists and has the SHA-1 `expected`.
pub fn file_matches(path: &Path, expected: &str) -> bool {
    sha1_of(path).is_ok_and(|hash| hash.eq_ignore_ascii_case(expected))
}

/// The SHA-1 of a file, as lowercase hex.
pub fn sha1_of(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// The SHA-1 of bytes, as lowercase hex.
pub fn sha1_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

#[cfg(unix)]
fn set_executable(item: &DownloadItem) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    if !item.executable {
        return Ok(());
    }
    let mut permissions = fs::metadata(&item.dest)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(&item.dest, permissions)
}

#[cfg(not(unix))]
fn set_executable(_: &DownloadItem) -> io::Result<()> {
    Ok(())
}

/// Drops repeated destinations (the same library listed twice, or two assets with
/// the same hash), keeping the first.
fn dedupe(items: Vec<DownloadItem>) -> Vec<DownloadItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.dest.clone()))
        .collect()
}

/// A [`Sleep`] that returns at once and records each wait, for tests.
pub fn recording_sleep() -> (Sleep, Arc<Mutex<Vec<Duration>>>) {
    let waits = Arc::new(Mutex::new(Vec::new()));
    let sleep: Sleep = {
        let waits = waits.clone();
        Arc::new(move |duration| {
            waits.lock().unwrap().push(duration);
            Box::pin(async {})
        })
    };
    (sleep, waits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, b"abc").unwrap();
        assert!(file_matches(
            &path,
            "A9993E364706816ABA3E25717850C26C9CD0D89D"
        ));
        assert!(!file_matches(&path, &sha1_hex(b"abd")));
        assert!(!file_matches(&dir.path().join("missing"), &sha1_hex(b"")));
    }

    #[test]
    fn dedupes_destinations() {
        let item = |url: &str, dest: &str| DownloadItem {
            url: url.into(),
            dest: dest.into(),
            sha1: String::new(),
            size: None,
            executable: false,
        };
        let items = dedupe(vec![item("a", "/x"), item("b", "/x"), item("c", "/y")]);
        assert_eq!(
            items.iter().map(|i| i.url.as_str()).collect::<Vec<_>>(),
            ["a", "c"]
        );
    }
}
