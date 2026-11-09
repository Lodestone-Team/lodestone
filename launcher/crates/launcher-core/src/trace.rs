//! The launcher's debug traces: one bus for the process, which `lodestone-launcher-cli
//! trace` reads over the control socket (this user only).
//!
//! Local debugging only. Events stay in memory (the last [`DEFAULT_BACKLOG`]) and go
//! to subscribers on the socket; nothing is written to disk or sent anywhere.
//!
//! What is traced, by topic:
//!
//! - `ui.*`: clicks on the shared controls (`ui.click`), shortcuts and the actions
//!   they run (`ui.action`), dialogs and popovers opening and closing (`ui.modal`,
//!   `ui.scope`), zoom; never typed text;
//! - `nav.*`: page, tab and selected instance;
//! - `settings.*`: a setting's new value when it is saved;
//! - `http.*`: method, host, path, status and time of each request (no query
//!   strings, headers or bodies); `download.file` for game files (live only);
//! - `game.*`: launch, state changes, exit; `game.log` for console lines (live only);
//! - `job.*`: the control API's jobs;
//! - `log.*`: warnings and errors the app logs (`log.debug` and `log.trace` live only);
//! - `perf.frame`: how long the window took to draw (live only).
//!
//! Payloads pass through [`lodestone_trace::scrub`]; callers leave secrets out too.

use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};

use futures::FutureExt as _;
use http_client::{AsyncBody, HttpClient, Response, Url, http::HeaderValue};
pub use lodestone_trace::{DEFAULT_BACKLOG, Event, Filter, Sink, TraceBus};
use serde_json::{Value, json};

/// Busy topics: sent only while a subscriber listens, and kept out of the backlog.
pub const LIVE_ONLY: &[&str] = &[
    "perf",
    "game.log",
    "download.file",
    "log.debug",
    "log.trace",
    "ui.key",
];

/// The launcher's bus.
pub fn bus() -> &'static TraceBus {
    static BUS: OnceLock<TraceBus> = OnceLock::new();
    BUS.get_or_init(|| TraceBus::new(DEFAULT_BACKLOG, LIVE_ONLY))
}

/// Emits an event; `data` is only made when it goes somewhere.
pub fn emit(topic: &str, data: impl FnOnce() -> Value) {
    bus().emit(topic, data);
}

/// Records what the launcher shows now, for `trace.snapshot`.
pub fn set_state(key: &str, value: Value) {
    bus().set_state(key, value);
}

/// The fields that differ between two JSON values, as (dotted path, before, after).
/// Objects are compared field by field; anything else as a whole.
pub fn diff(before: &Value, after: &Value) -> Vec<(String, Value, Value)> {
    fn walk(path: &str, before: &Value, after: &Value, out: &mut Vec<(String, Value, Value)>) {
        match (before, after) {
            (Value::Object(a), Value::Object(b)) => {
                let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
                keys.sort();
                keys.dedup();
                for key in keys {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    let (x, y) = (
                        a.get(key).unwrap_or(&Value::Null),
                        b.get(key).unwrap_or(&Value::Null),
                    );
                    walk(&child, x, y, out);
                }
            }
            _ if before != after => out.push((path.to_string(), before.clone(), after.clone())),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk("", before, after, &mut out);
    out
}

/// Emits `settings.change` for each field that differs between `before` and `after`
/// (any serializable settings), with `scope` ("launcher", "instance", "shared") and
/// `extra` fields (such as the instance) in each event.
pub fn settings_changed(
    scope: &str,
    before: &impl serde::Serialize,
    after: &impl serde::Serialize,
    extra: Value,
) {
    if !bus().wants("settings.change") {
        return;
    }
    let (Ok(before), Ok(after)) = (serde_json::to_value(before), serde_json::to_value(after))
    else {
        return;
    };
    for (path, from, to) in diff(&before, &after) {
        emit("settings.change", || {
            let mut event = json!({ "scope": scope, "path": path, "from": from, "to": to });
            if let (Value::Object(event), Value::Object(extra)) = (&mut event, &extra) {
                event.extend(extra.clone());
            }
            event
        });
    }
}

/// A URL's host and path, without its query or credentials.
pub fn url_parts(url: &str) -> (String, String) {
    match Url::parse(url) {
        Ok(url) => (
            url.host_str().unwrap_or_default().to_string(),
            url.path().to_string(),
        ),
        Err(_) => (
            String::new(),
            url.split('?').next().unwrap_or_default().to_string(),
        ),
    }
}

/// An [`HttpClient`] that emits an event for each request: `{prefix}.response` with
/// method, host, path, status and milliseconds, or `{prefix}.error`.
pub struct TracedClient {
    inner: Arc<dyn HttpClient>,
    /// `http` for API calls; `download` for game files, whose successes are
    /// `download.file` (live only).
    prefix: &'static str,
}

impl TracedClient {
    pub fn wrap(inner: Arc<dyn HttpClient>, prefix: &'static str) -> Arc<dyn HttpClient> {
        Arc::new(Self { inner, prefix })
    }
}

impl HttpClient for TracedClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        self.inner.user_agent()
    }

    fn proxy(&self) -> Option<&Url> {
        self.inner.proxy()
    }

    fn send(
        &self,
        req: http_client::http::Request<AsyncBody>,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let method = req.method().to_string();
        let host = req.uri().host().unwrap_or_default().to_string();
        let path = req.uri().path().to_string();
        let prefix = self.prefix;
        let started = Instant::now();
        self.inner
            .send(req)
            .map(move |result| {
                let ms = started.elapsed().as_millis() as u64;
                match &result {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let topic = if prefix == "download" && status < 400 {
                            "download.file".to_string()
                        } else {
                            format!("{prefix}.response")
                        };
                        emit(&topic, || {
                            json!({ "method": method, "host": host, "path": path, "status": status, "ms": ms })
                        });
                    }
                    Err(err) => {
                        let error = format!("{err:#}");
                        emit(&format!("{prefix}.error"), || {
                            json!({ "method": method, "host": host, "path": path, "error": error, "ms": ms })
                        });
                    }
                }
                result
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_name_each_changed_field() {
        let before =
            json!({ "language": "system", "defaults": { "memory": 4096, "args": "" }, "gone": 1 });
        let after = json!({ "language": "fr", "defaults": { "memory": 4096, "args": "-Xss2M" }, "new": true });
        let paths: Vec<String> = diff(&before, &after).into_iter().map(|d| d.0).collect();
        assert_eq!(paths, ["defaults.args", "gone", "language", "new"]);
    }

    #[test]
    fn urls_lose_their_query() {
        assert_eq!(
            url_parts("https://api.modrinth.com/v2/search?query=sodium&key=abc"),
            ("api.modrinth.com".into(), "/v2/search".into())
        );
        assert_eq!(
            url_parts("not a url?x=1"),
            (String::new(), "not a url".into())
        );
    }
}
