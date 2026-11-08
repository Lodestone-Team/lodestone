//! The HTTP client for the Modrinth API, with a shared cache and rate limit handling.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use axum::body::Bytes;
use color_eyre::eyre::eyre;
use reqwest::header::HeaderMap;
use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use sha1::Digest;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use super::cache::TtlCache;
use super::search::SearchParams;
use super::types::{
    ModrinthProject, ModrinthSearchResponse, ModrinthVersion, RawProject, RawVersion,
};
use super::{FileHashes, ModrinthApi, API_BASE, USER_AGENT};
use crate::error::{Error, ErrorKind};

/// Modrinth API requests time out after this long.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// A Modrinth API response may be at most this large.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
/// Search, project and version responses are cached this long.
const RESPONSE_TTL: Duration = Duration::from_secs(5 * 60);
/// Which version a file hash belongs to is cached this long.
const HASH_TTL: Duration = Duration::from_secs(60 * 60);
/// The newest version for a file hash is cached as long as other version responses.
const UPDATE_TTL: Duration = RESPONSE_TTL;
/// When the rate limit is used up for at most this long, requests wait; otherwise they fail.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(5);
/// Modrinth takes at most this many hashes or ids in one request.
const BATCH_SIZE: usize = 100;

fn unreachable(message: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::External,
        source: eyre!("Modrinth is not reachable: {message}"),
    }
}

/// The Modrinth API client. One is shared by all users, so the cache and the rate limit
/// apply to the whole core.
pub struct ModrinthClient {
    api: reqwest::Client,
    downloads: reqwest::Client,
    base_url: url::Url,
    responses: Mutex<TtlCache>,
    hashes: Mutex<TtlCache>,
    /// Set when Modrinth says the rate limit is used up.
    blocked_until: Mutex<Option<Instant>>,
}

impl Default for ModrinthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ModrinthClient {
    pub fn new() -> Self {
        Self::with_clients(
            Self::api_client(),
            Self::download_client(),
            url::Url::parse(API_BASE).expect("API_BASE is a valid URL"),
        )
    }

    fn api_client() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(REQUEST_TIMEOUT)
    }

    /// No overall timeout for downloads, which can be large, but a stalled one ends.
    fn download_client() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
    }

    /// A client for a test server on loopback at `base_url`. It ignores proxy settings,
    /// since it never leaves this machine.
    #[cfg(test)]
    pub(crate) fn for_test_server(base_url: url::Url) -> Self {
        Self::with_clients(
            Self::api_client().no_proxy(),
            Self::download_client().no_proxy(),
            base_url,
        )
    }

    fn with_clients(
        api: reqwest::ClientBuilder,
        downloads: reqwest::ClientBuilder,
        base_url: url::Url,
    ) -> Self {
        Self {
            api: api.build().unwrap_or_else(|_| reqwest::Client::new()),
            downloads: downloads.build().unwrap_or_else(|_| reqwest::Client::new()),
            base_url,
            responses: Mutex::new(TtlCache::new(500, 64 * 1024 * 1024)),
            hashes: Mutex::new(TtlCache::new(5000, 16 * 1024 * 1024)),
            blocked_until: Mutex::new(None),
        }
    }

    fn url(&self, segments: &[&str], query: &[(&str, String)]) -> url::Url {
        let mut url = self.base_url.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        url
    }

    fn cache_get(cache: &Mutex<TtlCache>, key: &str) -> Option<Bytes> {
        cache.lock().unwrap_or_else(|e| e.into_inner()).get(key)
    }

    fn cache_put(cache: &Mutex<TtlCache>, key: String, value: Bytes, ttl: Duration) {
        cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, value, ttl);
    }

    /// Waits a little, or fails, while Modrinth's rate limit is used up.
    async fn wait_for_rate_limit(&self) -> Result<(), Error> {
        let wait = {
            let mut blocked = self.blocked_until.lock().unwrap_or_else(|e| e.into_inner());
            match *blocked {
                Some(until) if until > Instant::now() => until - Instant::now(),
                Some(_) => {
                    *blocked = None;
                    return Ok(());
                }
                None => return Ok(()),
            }
        };
        if wait > MAX_RATE_LIMIT_WAIT {
            return Err(unreachable(format!(
                "the rate limit was reached, try again in {} seconds",
                wait.as_secs().max(1)
            )));
        }
        tokio::time::sleep(wait).await;
        Ok(())
    }

    /// Reads the `X-Ratelimit-*` headers. When no requests are left, later requests wait
    /// until the limit resets.
    fn note_rate_limit(&self, headers: &HeaderMap, status: StatusCode) {
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
        };
        let exhausted =
            status == StatusCode::TOO_MANY_REQUESTS || header("x-ratelimit-remaining") == Some(0);
        if !exhausted {
            return;
        }
        let reset = header("x-ratelimit-reset")
            .or_else(|| header("retry-after"))
            .unwrap_or(60)
            .clamp(1, 120);
        warn!("The Modrinth rate limit was reached; requests are paused for {reset} seconds");
        *self.blocked_until.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Instant::now() + Duration::from_secs(reset));
    }

    /// Sends a request and reads the body, mapping Modrinth's errors.
    async fn send(&self, request: RequestBuilder) -> Result<Bytes, Error> {
        self.wait_for_rate_limit().await?;
        let mut response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                unreachable("the request timed out")
            } else {
                unreachable(e.without_url())
            }
        })?;
        let status = response.status();
        self.note_rate_limit(response.headers(), status);
        if status == StatusCode::NOT_FOUND {
            return Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Not found on Modrinth"),
            });
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(unreachable("too many requests, try again in a minute"));
        }
        if status.is_server_error() {
            return Err(unreachable(format!("it answered {status}")));
        }
        if !status.is_success() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Modrinth rejected the request ({status})"),
            });
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_RESPONSE_BYTES as u64)
        {
            return Err(unreachable("the response is too large"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| unreachable(e.without_url()))?
        {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(unreachable("the response is too large"));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Bytes::from(body))
    }

    /// A GET request whose response is cached for `ttl`.
    async fn get_cached(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
        ttl: Duration,
    ) -> Result<Bytes, Error> {
        let url = self.url(segments, query);
        let key = url.to_string();
        if let Some(bytes) = Self::cache_get(&self.responses, &key) {
            return Ok(bytes);
        }
        let bytes = self.send(self.api.get(url)).await?;
        Self::cache_put(&self.responses, key, bytes.clone(), ttl);
        Ok(bytes)
    }

    async fn post(&self, segments: &[&str], body: &serde_json::Value) -> Result<Bytes, Error> {
        let url = self.url(segments, &[]);
        self.send(self.api.post(url).json(body)).await
    }

    /// The cache key for a project fetched by id. `projects` fills the same entries.
    fn project_key(&self, id: &str) -> String {
        self.url(&["project", id], &[]).to_string()
    }

    /// Looks up hashes in the hash cache under `prefix`, asks Modrinth for the rest with
    /// `fetch`, and caches every answer, including "not found". `hashes` must be lowercase
    /// and without duplicates (see [`normalized_hashes`]).
    async fn by_hash<F, Fut>(
        &self,
        prefix: &str,
        hashes: &[String],
        ttl: Duration,
        fetch: F,
    ) -> Result<HashMap<String, ModrinthVersion>, Error>
    where
        F: Fn(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = Result<HashMap<String, RawVersion>, Error>>,
    {
        let mut found = HashMap::new();
        let mut missing: Vec<String> = Vec::new();
        for hash in hashes {
            match Self::cache_get(&self.hashes, &format!("{prefix}{hash}"))
                .and_then(|b| serde_json::from_slice::<Option<ModrinthVersion>>(&b).ok())
            {
                Some(Some(version)) => {
                    found.insert(hash.clone(), version);
                }
                Some(None) => {}
                None => missing.push(hash.clone()),
            }
        }
        for chunk in missing.chunks(BATCH_SIZE) {
            let answer: HashMap<String, ModrinthVersion> = fetch(chunk.to_vec())
                .await?
                .into_iter()
                .map(|(hash, raw)| (hash.to_lowercase(), ModrinthVersion::from(raw)))
                .collect();
            for hash in chunk {
                let version = answer.get(hash).cloned();
                if let Ok(bytes) = serde_json::to_vec(&version) {
                    Self::cache_put(
                        &self.hashes,
                        format!("{prefix}{hash}"),
                        Bytes::from(bytes),
                        ttl,
                    );
                }
                if let Some(version) = version {
                    found.insert(hash.clone(), version);
                }
            }
        }
        Ok(found)
    }
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|e| unreachable(format!("unexpected response ({e})")))
}

/// Lowercases and de-duplicates hashes, keeping their order.
fn normalized_hashes(hashes: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(hashes.len());
    for hash in hashes {
        let hash = hash.to_lowercase();
        if !out.contains(&hash) {
            out.push(hash);
        }
    }
    out
}

#[async_trait]
impl ModrinthApi for ModrinthClient {
    async fn search(&self, params: &SearchParams) -> Result<ModrinthSearchResponse, Error> {
        let bytes = self
            .get_cached(&["search"], &params.query_pairs(), RESPONSE_TTL)
            .await?;
        Ok(parse::<ModrinthSearchResponse>(&bytes)?.normalize())
    }

    async fn project(&self, id_or_slug: &str) -> Result<ModrinthProject, Error> {
        let bytes = self
            .get_cached(&["project", id_or_slug], &[], RESPONSE_TTL)
            .await?;
        Ok(parse::<RawProject>(&bytes)?.into())
    }

    async fn projects(&self, ids: &[String]) -> Result<Vec<ModrinthProject>, Error> {
        let mut projects = Vec::new();
        let mut missing = Vec::new();
        for id in ids {
            match Self::cache_get(&self.responses, &self.project_key(id))
                .and_then(|b| serde_json::from_slice::<RawProject>(&b).ok())
            {
                Some(raw) => projects.push(raw.into()),
                None if !missing.contains(id) => missing.push(id.clone()),
                None => {}
            }
        }
        for chunk in missing.chunks(BATCH_SIZE) {
            let ids = serde_json::to_string(chunk).unwrap_or_default();
            let url = self.url(&["projects"], &[("ids", ids)]);
            let bytes = self.send(self.api.get(url)).await?;
            for value in parse::<Vec<serde_json::Value>>(&bytes)? {
                let Ok(raw) = serde_json::from_value::<RawProject>(value.clone()) else {
                    continue;
                };
                if let Ok(bytes) = serde_json::to_vec(&value) {
                    Self::cache_put(
                        &self.responses,
                        self.project_key(&raw.id),
                        Bytes::from(bytes),
                        RESPONSE_TTL,
                    );
                }
                projects.push(raw.into());
            }
        }
        Ok(projects)
    }

    async fn project_versions(
        &self,
        id_or_slug: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ModrinthVersion>, Error> {
        let mut query = Vec::new();
        if !loaders.is_empty() {
            query.push((
                "loaders",
                serde_json::to_string(loaders).unwrap_or_default(),
            ));
        }
        if !game_versions.is_empty() {
            query.push((
                "game_versions",
                serde_json::to_string(game_versions).unwrap_or_default(),
            ));
        }
        let bytes = self
            .get_cached(&["project", id_or_slug, "version"], &query, RESPONSE_TTL)
            .await?;
        let mut versions: Vec<ModrinthVersion> = parse::<Vec<RawVersion>>(&bytes)?
            .into_iter()
            .map(Into::into)
            .collect();
        // RFC 3339 dates in the same format sort as text
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(versions)
    }

    async fn version(&self, version_id: &str) -> Result<ModrinthVersion, Error> {
        let bytes = self
            .get_cached(&["version", version_id], &[], RESPONSE_TTL)
            .await?;
        Ok(parse::<RawVersion>(&bytes)?.into())
    }

    async fn project_version(
        &self,
        project: &str,
        id_or_number: &str,
    ) -> Result<ModrinthVersion, Error> {
        let bytes = self
            .get_cached(
                &["project", project, "version", id_or_number],
                &[],
                RESPONSE_TTL,
            )
            .await?;
        Ok(parse::<RawVersion>(&bytes)?.into())
    }

    async fn versions_by_hash(
        &self,
        sha512: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let hashes = normalized_hashes(sha512);
        self.by_hash("version:", &hashes, HASH_TTL, |chunk| async move {
            let body = serde_json::json!({ "hashes": chunk, "algorithm": "sha512" });
            parse(&self.post(&["version_files"], &body).await?)
        })
        .await
    }

    async fn latest_versions_by_hash(
        &self,
        sha512: &[String],
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let hashes = normalized_hashes(sha512);
        let prefix = format!("update:{}:{}:", loaders.join(","), game_versions.join(","));
        self.by_hash(&prefix, &hashes, UPDATE_TTL, |chunk| async move {
            let body = serde_json::json!({
                "hashes": chunk,
                "algorithm": "sha512",
                "loaders": loaders,
                "game_versions": game_versions,
            });
            parse(&self.post(&["version_files", "update"], &body).await?)
        })
        .await
    }

    async fn download(
        &self,
        url: &url::Url,
        file: &mut tokio::fs::File,
        max_bytes: u64,
    ) -> Result<FileHashes, Error> {
        let host = url.host_str().unwrap_or_default().to_string();
        let failed = |message: String| Error {
            kind: ErrorKind::External,
            source: eyre!("Download from {host} failed: {message}"),
        };
        let mut response = self
            .downloads
            .get(url.clone())
            .send()
            .await
            .map_err(|e| failed(e.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(failed(format!("the server answered {status}")));
        }
        if response.content_length().is_some_and(|len| len > max_bytes) {
            return Err(too_large(max_bytes));
        }
        let mut sha1 = sha1::Sha1::new();
        let mut sha512 = sha2::Sha512::new();
        let mut size = 0_u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| failed(e.without_url().to_string()))?
        {
            size += chunk.len() as u64;
            if size > max_bytes {
                return Err(too_large(max_bytes));
            }
            sha1.update(&chunk);
            sha512.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|e| eyre!("Failed to write the download: {e}"))?;
        }
        file.flush()
            .await
            .map_err(|e| eyre!("Failed to write the download: {e}"))?;
        Ok(FileHashes {
            size,
            sha1: hex::encode(sha1.finalize()),
            sha512: hex::encode(sha512.finalize()),
        })
    }
}

/// The error for a file that is larger than allowed.
pub fn too_large(max_bytes: u64) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(
            "The file is larger than {}",
            crate::util::format_byte(max_bytes)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_urls() {
        let client = ModrinthClient::new();
        assert_eq!(
            client
                .url(&["project", "fabric-api", "version"], &[])
                .as_str(),
            "https://api.modrinth.com/v2/project/fabric-api/version"
        );
        // a segment can never add path parts
        assert_eq!(
            client.url(&["project", "a/b?c#d"], &[]).as_str(),
            "https://api.modrinth.com/v2/project/a%2Fb%3Fc%23d"
        );
        let url = client.url(
            &["project", "x", "version"],
            &[("loaders", r#"["fabric"]"#.to_string())],
        );
        assert_eq!(
            url.query_pairs().next().unwrap().1,
            r#"["fabric"]"#,
            "{url}"
        );
    }

    #[test]
    fn rate_limit_headers_pause_requests() {
        let client = ModrinthClient::new();
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "12".parse().unwrap());
        client.note_rate_limit(&headers, StatusCode::OK);
        assert!(client.blocked_until.lock().unwrap().is_none());

        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("x-ratelimit-reset", "30".parse().unwrap());
        client.note_rate_limit(&headers, StatusCode::OK);
        let until = client.blocked_until.lock().unwrap().unwrap();
        assert!(until > Instant::now() + Duration::from_secs(25));
    }

    /// A request the test server received.
    #[derive(Debug, Clone)]
    struct Received {
        method: String,
        path: String,
        query: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Received {
        fn query(&self, key: &str) -> Option<&str> {
            self.query
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        }
    }

    type Reply = (StatusCode, HeaderMap, Vec<u8>);

    fn json(body: &str) -> Reply {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        (StatusCode::OK, headers, body.as_bytes().to_vec())
    }

    fn status(status: StatusCode) -> Reply {
        (status, HeaderMap::new(), Vec::new())
    }

    /// An HTTP server on loopback that answers with `respond` and records every request,
    /// with a client whose API base URL points at it.
    struct TestServer {
        client: ModrinthClient,
        base: url::Url,
        received: std::sync::Arc<Mutex<Vec<Received>>>,
    }

    impl TestServer {
        async fn start(respond: impl Fn(&Received) -> Reply + Send + Sync + 'static) -> Self {
            use axum::http::{Method, Uri};
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base =
                url::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
            let received = std::sync::Arc::new(Mutex::new(Vec::new()));
            let respond = std::sync::Arc::new(respond);
            let log = received.clone();
            let app = axum::Router::new().fallback(
                move |method: Method, uri: Uri, body: Bytes| async move {
                    let request = Received {
                        method: method.to_string(),
                        path: uri.path().to_string(),
                        query: url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
                            .into_owned()
                            .collect(),
                        body: body.to_vec(),
                    };
                    let reply = respond(&request);
                    log.lock().unwrap().push(request);
                    reply
                },
            );
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            TestServer {
                client: ModrinthClient::for_test_server(base.join("v2").unwrap()),
                base,
                received,
            }
        }

        fn received(&self) -> Vec<Received> {
            self.received.lock().unwrap().clone()
        }
    }

    const SEARCH: &str = include_str!("testdata/search_fabric_api.json");
    const PROJECT: &str = include_str!("testdata/project_fabric_api.json");
    const VERSION: &str = include_str!("testdata/version_lazydfu_0.1.3.json");
    const VERSIONS: &str = include_str!("testdata/versions_lazydfu.json");
    const VERSION_FILES: &str = include_str!("testdata/version_files_lazydfu.json");
    const LAZYDFU_SHA512: &str = "dc3766352c645f6da92b13000dffa80584ee58093c925c2154eb3c125a2b2f9a3af298202e2658b039c6ee41e81ca9a2e9d4b942561f7085239dd4421e0cce0a";

    fn fabric_api_search() -> SearchParams {
        SearchParams::new(
            Some("fabric-api".into()),
            crate::modrinth::ModrinthProjectType::Mod,
            Some("1.20.1".into()),
            Some("fabric".into()),
            None,
            None,
            Some(10),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn search_sends_the_query_and_reads_the_hits() {
        let server = TestServer::start(|r| match r.path.as_str() {
            "/v2/search" => json(SEARCH),
            _ => status(StatusCode::NOT_FOUND),
        })
        .await;
        let params = fabric_api_search();
        let response = server.client.search(&params).await.unwrap();
        assert_eq!(response.total_hits, 44);
        let hit = response
            .hits
            .iter()
            .find(|h| h.slug == "fabric-api")
            .expect("fabric-api is found");
        assert_eq!(hit.project_id, "P7dR8mSH");
        assert!(hit.categories.contains(&"fabric".to_string()));

        let received = server.received();
        assert_eq!(received.len(), 1);
        let request = &received[0];
        assert_eq!(request.method, "GET");
        assert_eq!(request.query("query"), Some("fabric-api"));
        assert_eq!(request.query("limit"), Some("10"));
        let facets: Vec<Vec<String>> =
            serde_json::from_str(request.query("facets").unwrap()).unwrap();
        assert_eq!(facets, params.facets());

        // the same search again is answered from the cache
        server.client.search(&params).await.unwrap();
        assert_eq!(server.received().len(), 1);
    }

    #[tokio::test]
    async fn resolves_a_link_to_the_project() {
        let server = TestServer::start(|r| match r.path.as_str() {
            "/v2/project/fabric-api" => json(PROJECT),
            "/v2/project/P7dR8mSH/version/0.1.3" => json(VERSION),
            _ => status(StatusCode::NOT_FOUND),
        })
        .await;
        let target = crate::modrinth::links::resolve_url(
            &server.client,
            "https://modrinth.com/mod/fabric-api",
        )
        .await
        .unwrap();
        assert_eq!(target.project_id, "P7dR8mSH");
        assert_eq!(target.slug, "fabric-api");
        assert_eq!(target.project_type, "mod");
        assert_eq!(target.version_id, None);

        // a version link looks the version up under the canonical project id
        let target = crate::modrinth::links::resolve_url(
            &server.client,
            "modrinth.com/mod/fabric-api/version/0.1.3",
        )
        .await
        .unwrap();
        assert_eq!(target.version_id.as_deref(), Some("4SHylIO9"));

        let err =
            crate::modrinth::links::resolve_url(&server.client, "https://modrinth.com/mod/nope")
                .await
                .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
    }

    #[tokio::test]
    async fn project_versions_are_filtered_and_newest_first() {
        let server = TestServer::start(|r| match r.path.as_str() {
            "/v2/project/lazydfu/version" => {
                // in another order than Modrinth sends them, to check the sorting
                let mut versions: Vec<serde_json::Value> = serde_json::from_str(VERSIONS).unwrap();
                versions.reverse();
                json(&serde_json::to_string(&versions).unwrap())
            }
            _ => status(StatusCode::NOT_FOUND),
        })
        .await;
        let versions = server
            .client
            .project_versions("lazydfu", &["fabric".into()], &["1.16.5".into()])
            .await
            .unwrap();
        let ids: Vec<_> = versions.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["4SHylIO9", "C6e265zK", "aBDhdWUD", "8tFjDrHf"]);
        let request = &server.received()[0];
        assert_eq!(request.query("loaders"), Some(r#"["fabric"]"#));
        assert_eq!(request.query("game_versions"), Some(r#"["1.16.5"]"#));
    }

    #[tokio::test]
    async fn versions_by_hash_are_looked_up_once() {
        let server = TestServer::start(|r| match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/v2/version/4SHylIO9") => json(VERSION),
            ("POST", "/v2/version_files") => json(VERSION_FILES),
            _ => status(StatusCode::NOT_FOUND),
        })
        .await;
        let version = server.client.version("4SHylIO9").await.unwrap();
        assert_eq!(version.project_id, "hvFnDODi");
        let file = version.primary_file().unwrap();
        assert_eq!(file.filename, "lazydfu-0.1.3.jar");
        assert_eq!(file.size, 14531);
        assert_eq!(file.sha512, LAZYDFU_SHA512);

        let unknown = "f".repeat(128);
        let hashes = [LAZYDFU_SHA512.to_uppercase(), unknown.clone()];
        let found = server.client.versions_by_hash(&hashes).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[LAZYDFU_SHA512].id, "4SHylIO9");
        let posts: Vec<_> = server
            .received()
            .into_iter()
            .filter(|r| r.method == "POST")
            .collect();
        assert_eq!(posts.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&posts[0].body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "hashes": [LAZYDFU_SHA512, unknown], "algorithm": "sha512" })
        );

        // both answers are cached, including "not found"
        let again = server.client.versions_by_hash(&hashes).await.unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(server.received().len(), 2);
    }

    #[tokio::test]
    async fn error_statuses_map_to_error_kinds() {
        let server = TestServer::start(|r| match r.path.as_str() {
            "/v2/version/missing" => status(StatusCode::NOT_FOUND),
            "/v2/version/broken" => status(StatusCode::BAD_GATEWAY),
            "/v2/version/rejected" => status(StatusCode::BAD_REQUEST),
            "/v2/version/garbage" => json("{ not json"),
            _ => status(StatusCode::IM_A_TEAPOT),
        })
        .await;
        let kind = |id: &'static str| {
            let client = &server.client;
            async move { client.version(id).await.unwrap_err().kind }
        };
        assert!(matches!(kind("missing").await, ErrorKind::NotFound));
        assert!(matches!(kind("broken").await, ErrorKind::External));
        assert!(matches!(kind("rejected").await, ErrorKind::BadRequest));
        assert!(matches!(kind("garbage").await, ErrorKind::External));
    }

    #[tokio::test]
    async fn too_many_requests_pauses_later_requests() {
        let server = TestServer::start(|_| {
            let mut headers = HeaderMap::new();
            headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
            headers.insert("x-ratelimit-reset", "30".parse().unwrap());
            (StatusCode::TOO_MANY_REQUESTS, headers, Vec::new())
        })
        .await;
        let err = server.client.version("4SHylIO9").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::External));
        assert!(
            err.source.to_string().contains("too many requests"),
            "{err:?}"
        );
        // the next request fails at once, without asking Modrinth again
        let err = server.client.project("lazydfu").await.unwrap_err();
        assert!(err
            .source
            .to_string()
            .starts_with("Modrinth is not reachable: the rate limit was reached"));
        assert_eq!(server.received().len(), 1);
    }

    #[tokio::test]
    async fn download_hashes_the_file_and_enforces_the_limit() {
        let content: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let served = content.clone();
        let server = TestServer::start(move |r| match r.path.as_str() {
            "/data/a.jar" => (StatusCode::OK, HeaderMap::new(), served.clone()),
            _ => status(StatusCode::NOT_FOUND),
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.jar");
        let url = server.base.join("data/a.jar").unwrap();

        let mut file = tokio::fs::File::create(&path).await.unwrap();
        let hashes = server
            .client
            .download(&url, &mut file, 1_000_000)
            .await
            .unwrap();
        drop(file);
        assert_eq!(hashes.size, content.len() as u64);
        assert_eq!(hashes.sha512, crate::modrinth::fake::sha512_hex(&content));
        assert_eq!(hashes.sha1, crate::modrinth::fake::sha1_hex(&content));
        assert_eq!(std::fs::read(&path).unwrap(), content);

        let mut file = tokio::fs::File::create(dir.path().join("b.jar"))
            .await
            .unwrap();
        let err = server
            .client
            .download(&url, &mut file, 1000)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));

        let missing = server.base.join("data/missing.jar").unwrap();
        let err = server
            .client
            .download(&missing, &mut file, 1000)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::External));
        assert!(err.source.to_string().contains("404"), "{err:?}");
    }

    #[tokio::test]
    async fn fails_fast_while_rate_limited() {
        let client = ModrinthClient::new();
        *client.blocked_until.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        let err = client.version("abc").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::External));
        assert!(err
            .source
            .to_string()
            .starts_with("Modrinth is not reachable: the rate limit was reached"));
    }

    // Contract checks against the real Modrinth API, with Lodestone's user agent and a
    // handful of requests in total. They check what only the live service can tell: that
    // its search filters and CDN still behave as the code expects. Run them with
    // `cargo test -- --ignored`.

    #[tokio::test]
    #[ignore = "network: Modrinth search still applies the loader, version and server-side facets"]
    async fn network_search_finds_fabric_api() {
        let client = ModrinthClient::new();
        let response = client.search(&fabric_api_search()).await.unwrap();
        assert!(response.total_hits > 0);
        let hit = response
            .hits
            .iter()
            .find(|h| h.slug == "fabric-api")
            .expect("fabric-api is found");
        assert_eq!(hit.project_id, "P7dR8mSH");
        assert!(hit.categories.contains(&"fabric".to_string()));
        for hit in &response.hits {
            assert!(
                hit.server_side == "required" || hit.server_side == "optional",
                "{} is client only",
                hit.slug
            );
        }
    }

    #[tokio::test]
    #[ignore = "network: Modrinth reports plugins as mods with plugin loader categories"]
    async fn network_plugin_search_returns_plugins() {
        let client = ModrinthClient::new();
        let params = SearchParams::new(
            None,
            crate::modrinth::ModrinthProjectType::Plugin,
            None,
            None,
            Some(crate::modrinth::ModrinthSearchIndex::Downloads),
            None,
            Some(20),
        )
        .unwrap();
        let response = client.search(&params).await.unwrap();
        assert!(!response.hits.is_empty());
        for hit in &response.hits {
            assert!(
                hit.categories
                    .iter()
                    .any(|c| crate::modrinth::search::PLUGIN_LOADERS.contains(&c.as_str())),
                "{} has no plugin loader: {:?}",
                hit.slug,
                hit.categories
            );
        }
    }

    #[tokio::test]
    #[ignore = "network: a mod installed from Modrinth's CDN matches the hashes the API reports"]
    async fn network_installs_a_small_mod() {
        use crate::mods::{InstalledModLoader, ModFolder};
        let client = ModrinthClient::new();
        let dir = tempfile::tempdir().unwrap();
        let folder = ModFolder {
            loader: InstalledModLoader::Fabric,
            game_version: "1.20.1".into(),
            dir: dir.path().join("mods"),
        };
        // LazyDFU 0.1.3, about 15 KB, without dependencies
        let version = client.version("4SHylIO9").await.unwrap();
        let project = client.project(&version.project_id).await.unwrap();
        assert_eq!(project.slug, "lazydfu");
        assert_eq!(project.project_type, "mod");
        let result = crate::mods::install::install(
            &client,
            &folder,
            version.clone(),
            &project,
            crate::mods::install::InstallOptions {
                install_dependencies: true,
                force: false,
            },
        )
        .await
        .unwrap();
        let file = version.primary_file().unwrap();
        assert_eq!(result.installed.len(), 1);
        let installed = &result.installed[0];
        assert_eq!(installed.file_name, file.filename);
        assert_eq!(installed.sha512, file.sha512);
        let on_disk = crate::mods::scan::sha512_of(&folder.dir.join(&file.filename)).unwrap();
        assert_eq!(on_disk, file.sha512);
        assert_eq!(installed.modrinth.as_ref().unwrap().slug, "lazydfu");
    }
}
