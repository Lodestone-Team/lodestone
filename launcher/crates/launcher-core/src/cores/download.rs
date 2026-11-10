//! A file the core lets download by a one-time key (`GET /file/{key}`), streamed into
//! a file on disk with a size limit. Lodestone Host downloads a server's files this way.

use std::{fs::File, path::Path};

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
