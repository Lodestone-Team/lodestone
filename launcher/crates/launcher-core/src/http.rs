//! A thin layer over Zed's `http_client`, which the sign-in and skin code share.
//!
//! The client is injected as an `Arc<dyn HttpClient>`, and every endpoint comes from
//! [`Endpoints`](crate::auth::Endpoints), so tests point everything at a loopback
//! mock server. Requests never follow redirects, carry [`USER_AGENT`] and time out.

use std::{sync::Arc, time::Duration};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use serde::Serialize;

use crate::{Secret, USER_AGENT};

/// How long a single request may take, including its body.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Responses larger than this are refused. The sign-in APIs answer with a few KiB
/// of JSON, and skins are small PNGs.
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// The client the app uses: Zed's reqwest client with rustls and the platform
/// certificate verifier, sending [`USER_AGENT`].
pub fn default_client() -> anyhow::Result<Arc<dyn HttpClient>> {
    Ok(Arc::new(
        reqwest_client::ReqwestClient::proxy_and_user_agent(None, USER_AGENT)?,
    ))
}

/// The client for game downloads: like [`default_client`], but with no deadline for
/// the whole request, since a Java runtime file can be over 100 MB. Instead, a read
/// that waits longer than [`DOWNLOAD_IDLE_TIMEOUT`] fails, and the download engine
/// retries it.
pub fn download_client() -> anyhow::Result<Arc<dyn HttpClient>> {
    Ok(Arc::new(
        reqwest_client::ReqwestClient::proxy_user_agent_and_read_timeout(
            None,
            USER_AGENT,
            Some(DOWNLOAD_IDLE_TIMEOUT),
        )?,
    ))
}

/// How long a download may go without receiving data.
pub const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Polls `future` inside the HTTP client's Tokio runtime. Reading the body of a
/// [`download_client`] response starts Tokio timers (the idle timeout), which need
/// that runtime's context; GPUI's executor and `block_on` don't provide one.
pub fn in_http_runtime<'a, T: 'a>(
    future: impl std::future::Future<Output = T> + Send + 'a,
) -> impl std::future::Future<Output = T> + Send + 'a {
    InHttpRuntime(Box::pin(future))
}

struct InHttpRuntime<'a, T>(futures::future::BoxFuture<'a, T>);

impl<T> std::future::Future for InHttpRuntime<'_, T> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<T> {
        let _runtime = reqwest_client::runtime().enter();
        self.0.as_mut().poll(cx)
    }
}

/// A response with its body read into memory.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn json<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(&self.body)
    }
}

/// Why a request failed before a response arrived.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("could not connect: {0:#}")]
    Transport(anyhow::Error),
    #[error("the response was larger than {MAX_RESPONSE_BYTES} bytes")]
    TooLarge,
}

fn builder(method: Method, url: &str) -> http_client::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .follow_redirects(RedirectPolicy::NoFollow)
        .timeout(REQUEST_TIMEOUT)
}

/// `POST` a form, as the Microsoft identity platform expects.
pub fn form(url: &str, fields: &[(&str, &str)]) -> anyhow::Result<Request<AsyncBody>> {
    let body = serde_urlencoded_body(fields);
    Ok(builder(Method::POST, url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(AsyncBody::from(body))?)
}

/// `POST` a JSON body.
pub fn json(url: &str, body: &impl Serialize) -> anyhow::Result<Request<AsyncBody>> {
    Ok(builder(Method::POST, url)
        .header("Content-Type", "application/json")
        .body(AsyncBody::from(serde_json::to_vec(body)?))?)
}

/// `GET`, with a bearer token if given.
pub fn get(url: &str, bearer: Option<&Secret>) -> anyhow::Result<Request<AsyncBody>> {
    let mut request = builder(Method::GET, url);
    if let Some(token) = bearer {
        request = request.header("Authorization", format!("Bearer {}", token.expose()));
    }
    Ok(request.body(AsyncBody::empty())?)
}

/// Sends the request and reads the body, up to [`MAX_RESPONSE_BYTES`].
pub async fn send(
    client: &dyn HttpClient,
    request: Request<AsyncBody>,
) -> Result<Response, RequestError> {
    let response = client
        .send(request)
        .await
        .map_err(RequestError::Transport)?;
    let status = response.status().as_u16();
    let mut body = Vec::new();
    let read = response
        .into_body()
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .await;
    read.map_err(|err| RequestError::Transport(err.into()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(RequestError::TooLarge);
    }
    Ok(Response { status, body })
}

fn serde_urlencoded_body(fields: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish()
}

/// Joins `path` onto a base URL, keeping the base's own path.
pub fn join(base: &url::Url, path: &str) -> String {
    format!(
        "{}/{}",
        base.as_str().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_paths() {
        let base = url::Url::parse("http://127.0.0.1:1234/login/").unwrap();
        assert_eq!(
            join(&base, "/consumers/x"),
            "http://127.0.0.1:1234/login/consumers/x"
        );
        let base = url::Url::parse("https://api.minecraftservices.com").unwrap();
        assert_eq!(
            join(&base, "minecraft/profile"),
            "https://api.minecraftservices.com/minecraft/profile"
        );
    }

    #[test]
    fn encodes_forms() {
        assert_eq!(
            serde_urlencoded_body(&[("scope", "XboxLive.signin offline_access"), ("a", "&=")]),
            "scope=XboxLive.signin+offline_access&a=%26%3D"
        );
    }
}
