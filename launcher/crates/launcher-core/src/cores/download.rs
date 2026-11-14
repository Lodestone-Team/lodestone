//! A file the core lets download by a one-time key (`GET /file/{key}`), streamed into
//! a file on disk with a size limit. Lodestone Host downloads a server's files this way.

use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpRequestExt as _, Method, RedirectPolicy, Request};

use super::{
    CoreClient, CoreError,
    client::{Auth, RawResponse, check},
};
use crate::{
    USER_AGENT,
    game::download::{CancelToken, Progress, ProgressSnapshot},
};

/// How long a download may take.
pub const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3 * 60 * 60);

impl CoreClient {
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

/// Has `write` save into a file next to `out`, then renames it over `out` once it
/// succeeds. When `write` fails, or its future is dropped partway, the partial file is
/// removed and `out` is left as it was.
pub(crate) async fn save_via_partial<F, Fut>(out: &Path, write: F) -> Result<u64, CoreError>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<u64, CoreError>>,
{
    let partial = PartialFile(partial_path(out));
    let saved = write(partial.0.clone()).await?;
    std::fs::rename(&partial.0, out)
        .map_err(|err| CoreError::Invalid(format!("{}: {err}", out.display())))?;
    std::mem::forget(partial);
    Ok(saved)
}

/// A file being downloaded, removed when dropped: on an error, or when the download
/// is cancelled by dropping its future.
struct PartialFile(PathBuf);

impl Drop for PartialFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

/// Where a download into `out` is written until it completes: a hidden sibling
/// named for `out`, this process and a counter, so it never clashes with a file the
/// user has, or with another download.
fn partial_path(out: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    out.with_file_name(format!(".{name}.{}-{n}.part", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_replaces_the_file_only_once_complete() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("Survival 1.zip");
        std::fs::write(&out, b"earlier copy").unwrap();
        let saved = futures::executor::block_on(save_via_partial(&out, |partial| async move {
            std::fs::write(&partial, b"new zip").unwrap();
            Ok(7)
        }))
        .unwrap();
        assert_eq!(saved, 7);
        assert_eq!(std::fs::read(&out).unwrap(), b"new zip");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_download_leaves_the_file_there_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("Survival 1.zip");
        std::fs::write(&out, b"earlier copy").unwrap();
        // the core failed before writing anything, and after writing some
        for wrote in [false, true] {
            let result =
                futures::executor::block_on(save_via_partial(&out, |partial| async move {
                    if wrote {
                        std::fs::write(&partial, b"half a zip").unwrap();
                    }
                    Err(CoreError::Invalid("no space left".into()))
                }));
            assert!(result.is_err());
            assert_eq!(std::fs::read(&out).unwrap(), b"earlier copy");
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn a_dropped_download_leaves_no_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("Survival 1.zip");
        let written = std::sync::Arc::new(std::sync::Mutex::new(None));
        let mut save = Box::pin(save_via_partial(&out, {
            let written = written.clone();
            |partial| async move {
                std::fs::write(&partial, b"half a zip").unwrap();
                *written.lock().unwrap() = Some(partial);
                futures::future::pending::<()>().await;
                Ok(0)
            }
        }));
        let waker = futures::task::noop_waker();
        let mut poll_cx = std::task::Context::from_waker(&waker);
        assert!(save.as_mut().poll(&mut poll_cx).is_pending());
        let partial = written.lock().unwrap().clone().unwrap();
        assert!(partial.exists());
        // leaving the page cancels the download by dropping it
        drop(save);
        assert!(!partial.exists());
        assert!(!out.exists());
    }

    #[test]
    fn partial_files_are_hidden_siblings_that_never_clash() {
        let out = Path::new("downloads").join("Survival 1.zip");
        let (a, b) = (partial_path(&out), partial_path(&out));
        assert_ne!(a, b);
        assert_ne!(a, out);
        assert_eq!(a.parent(), out.parent());
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(".Survival 1.zip."), "{name}");
        assert!(name.ends_with(".part"), "{name}");
    }
}
