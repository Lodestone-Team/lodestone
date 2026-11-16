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
//!
//! Failures also go to the log file ([`crate::logging`]), whether traces are on or
//! not: [`emit`] logs an event that [`is_failure`] picks at warn, a few a minute per
//! topic. A request that couldn't reach its host is logged once per outage: the
//! first failure, then nothing for that host until it answers again (a saved core
//! that is off is polled every few seconds, and that's normal); a host that keeps
//! failing and answering by turns (flaky Wi-Fi) gets one line a minute at most. Game
//! file downloads (`download.*`) aren't logged from here: the downloader retries
//! failed attempts, and logs a file it gave up on itself.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
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

/// Emits an event; `data` is only made when it goes somewhere. An event on a topic
/// that can report a failure ([`may_fail`]) is made to find out, and a failure
/// ([`is_failure`]) is also logged, scrubbed, as `{topic} {data}` with the target
/// [`logging::TRACE_TARGET`], which the logger doesn't send back to the bus. Other
/// topics (clicks, the pointer, navigation) and live-only ones cost nothing while
/// traces are off.
///
/// [`logging::TRACE_TARGET`]: crate::logging::TRACE_TARGET
pub fn emit(topic: &str, data: impl FnOnce() -> Value) {
    static LIVE: OnceLock<Filter> = OnceLock::new();
    static THROTTLE: crate::logging::Throttle = crate::logging::Throttle::new();
    let bus = bus();
    let enabled = || log::log_enabled!(target: crate::logging::TRACE_TARGET, log::Level::Warn);
    // `log.*` events are the logger's own records; game downloads log their own.
    let may_log = may_fail(topic)
        && !topic.starts_with("log.")
        && !topic.starts_with("download.")
        && !LIVE
            .get_or_init(|| Filter::new(LIVE_ONLY.iter().copied()))
            .matches(topic)
        && enabled();
    if !may_log {
        bus.emit(topic, data);
        return;
    }
    let data = data();
    let host = data["host"].as_str().filter(|host| !host.is_empty());
    let unreachable = topic.ends_with(".error") && host.is_some();
    if is_failure(topic, &data) {
        let mut scrubbed = data.clone();
        lodestone_trace::scrub(&mut scrubbed);
        let outage = host
            .filter(|_| unreachable)
            .map(|host| (host, OUTAGES.begin(host, Instant::now())));
        match outage {
            Some((host, Outage::Started)) => log::warn!(
                target: crate::logging::TRACE_TARGET,
                "{topic} {scrubbed} (more failed requests to {host} aren't logged until it answers)"
            ),
            Some((_, Outage::Quiet)) => {}
            Some((host, Outage::Untracked)) => THROTTLE.log(
                host,
                log::Level::Warn,
                crate::logging::TRACE_TARGET,
                format_args!("{topic} {scrubbed}"),
            ),
            _ => THROTTLE.log(
                topic,
                log::Level::Warn,
                crate::logging::TRACE_TARGET,
                format_args!("{topic} {scrubbed}"),
            ),
        }
    } else if let Some(host) = host
        && topic.ends_with(".response")
        && OUTAGES.end(host)
    {
        log::info!(target: crate::logging::TRACE_TARGET, "{host} answers again");
    }
    // The bus scrubs what it keeps.
    bus.emit(topic, || data);
}

/// The hosts whose requests are failing, so a failure is logged once per outage.
static OUTAGES: Outages = Outages::new();

/// What a failed request to a host means for the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outage {
    /// It was answering: log the failure, and "answers again" when it does.
    Started,
    /// Already failing, or its last outage was logged less than [`Outages::QUIET`]
    /// ago: nothing now, nor when it answers.
    Quiet,
    /// Too many hosts to remember: log it through the throttle.
    Untracked,
}

struct Host {
    /// Its requests are failing.
    down: bool,
    /// This outage's start was logged, so its end is too.
    logged: bool,
    /// When an outage of it was last logged.
    logged_at: Instant,
}

/// Hosts that couldn't be reached since they last answered, and hosts whose last
/// outage was logged lately. Bounded at [`Outages::MAX`] hosts.
struct Outages {
    hosts: Mutex<BTreeMap<String, Host>>,
    /// Some host is down: a success needs looking at.
    any: AtomicBool,
}

impl Outages {
    const MAX: usize = 256;
    /// A host's outages are logged at most this often.
    const QUIET: Duration = Duration::from_secs(60);

    const fn new() -> Self {
        Self {
            hosts: Mutex::new(BTreeMap::new()),
            any: AtomicBool::new(false),
        }
    }

    fn any(&self) -> bool {
        self.any.load(Ordering::Relaxed)
    }

    /// A request to `host` failed at `now`.
    fn begin(&self, host: &str, now: Instant) -> Outage {
        let mut hosts = self.hosts.lock().unwrap_or_else(|p| p.into_inner());
        if !hosts.contains_key(host) && hosts.len() >= Self::MAX {
            hosts.retain(|_, h| h.down || now.duration_since(h.logged_at) < Self::QUIET);
            if hosts.len() >= Self::MAX {
                return Outage::Untracked;
            }
        }
        self.any.store(true, Ordering::Relaxed);
        match hosts.get_mut(host) {
            Some(known) if known.down => Outage::Quiet,
            Some(known) if now.duration_since(known.logged_at) < Self::QUIET => {
                known.down = true;
                known.logged = false;
                Outage::Quiet
            }
            Some(known) => {
                *known = Host {
                    down: true,
                    logged: true,
                    logged_at: now,
                };
                Outage::Started
            }
            None => {
                hosts.insert(
                    host.to_string(),
                    Host {
                        down: true,
                        logged: true,
                        logged_at: now,
                    },
                );
                Outage::Started
            }
        }
    }

    /// `host` answered: whether that ends an outage whose start was logged.
    fn end(&self, host: &str) -> bool {
        if !self.any() {
            return false;
        }
        let mut hosts = self.hosts.lock().unwrap_or_else(|p| p.into_inner());
        let logged = match hosts.get_mut(host) {
            Some(known) if known.down => {
                known.down = false;
                std::mem::take(&mut known.logged)
            }
            _ => false,
        };
        self.any
            .store(hosts.values().any(|h| h.down), Ordering::Relaxed);
        logged
    }
}

/// Whether events on `topic` can be failures ([`is_failure`]): errors, failed things,
/// responses (a server error), finished jobs, exits (a crash), and the storage,
/// image, snapshot and world backup topics, which carry an `error` or `warning`.
pub fn may_fail(topic: &str) -> bool {
    let last = topic.rsplit('.').next().unwrap_or(topic);
    matches!(last, "error" | "failed" | "response" | "finished" | "exit")
        || ["storage.", "images.", "snapshot.", "world_backup"]
            .iter()
            .any(|prefix| topic.starts_with(prefix))
}

/// Whether an event reports a failure: a topic ending in `error` or `failed`
/// (`http.error`, `game.failed`), data with an `error` or `warning` (`storage.move`,
/// `images.*`), a failed job, a game that crashed, or a server error answering a
/// request.
pub fn is_failure(topic: &str, data: &Value) -> bool {
    let last = topic.rsplit('.').next().unwrap_or(topic);
    let present = |key: &str| data.get(key).is_some_and(|v| !v.is_null() && v != false);
    match topic {
        // a cancelled job has an error too, but isn't a failure
        "job.finished" => data["state"] == "failed",
        "game.exit" => data["crashed"] == true,
        _ => {
            matches!(last, "error" | "failed")
                || present("error")
                || present("warning")
                || (last == "response" && data["status"].as_u64().is_some_and(|s| s >= 500))
        }
    }
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
    fn failures_are_picked_out() {
        let failures = [
            (
                "http.error",
                json!({ "host": "api.modrinth.com", "error": "timed out" }),
            ),
            ("core_http.error", json!({})),
            ("game.failed", json!({ "instance": "a" })),
            ("mod_finder.failed", json!({})),
            (
                "job.finished",
                json!({ "state": "failed", "method": "play.start" }),
            ),
            ("storage.move", json!({ "error": "disk full" })),
            ("images.content", json!({ "warning": "couldn't record" })),
            ("http.response", json!({ "status": 503 })),
            ("game.exit", json!({ "crashed": true, "code": 1 })),
        ];
        for (topic, data) in failures {
            assert!(is_failure(topic, &data), "{topic} {data}");
            assert!(may_fail(topic), "{topic}");
        }
        let fine = [
            ("job.finished", json!({ "state": "succeeded" })),
            (
                "job.finished",
                json!({ "state": "cancelled", "error": { "code": -32800 } }),
            ),
            ("storage.move", json!({ "error": null, "done": 3 })),
            ("http.response", json!({ "status": 404 })),
            ("game.exit", json!({ "crashed": false, "code": 0 })),
            ("ui.click", json!({ "id": "play" })),
            ("images.content", json!({ "warning": false })),
        ];
        for (topic, data) in fine {
            assert!(!is_failure(topic, &data), "{topic} {data}");
        }
        // Busy topics that never fail aren't made while traces are off.
        for topic in ["ui.click", "ui.pointer", "nav.page", "game.launch"] {
            assert!(!may_fail(topic), "{topic}");
        }
    }

    #[test]
    fn an_outage_is_counted_once() {
        let outages = Outages::new();
        let now = Instant::now();
        assert!(!outages.end("core.example"), "nothing to end");
        assert_eq!(outages.begin("core.example", now), Outage::Started);
        assert_eq!(
            outages.begin("core.example", now),
            Outage::Quiet,
            "the same outage"
        );
        assert_eq!(
            outages.begin("api.modrinth.com", now),
            Outage::Started,
            "another host"
        );
        assert!(outages.end("core.example"));
        assert!(!outages.end("core.example"));
        assert!(outages.any());
        assert!(outages.end("api.modrinth.com"));
        assert!(!outages.any());
        let later = now + Outages::QUIET;
        assert_eq!(
            outages.begin("core.example", later),
            Outage::Started,
            "a new outage a minute on"
        );
        for n in 0..Outages::MAX * 2 {
            outages.begin(&format!("host-{n}"), later);
        }
        assert!(outages.hosts.lock().unwrap().len() <= Outages::MAX);
        assert_eq!(
            outages.begin("one-too-many", later),
            Outage::Untracked,
            "logged through the throttle"
        );
    }

    #[test]
    fn a_flaky_host_is_logged_once_a_minute() {
        let outages = Outages::new();
        let start = Instant::now();
        let mut started = 0;
        let mut ended = 0;
        // error, success, error, success… for 59 s
        for second in 0..59 {
            let now = start + Duration::from_secs(second);
            started += usize::from(outages.begin("resources.example", now) == Outage::Started);
            ended += usize::from(outages.end("resources.example"));
        }
        assert_eq!((started, ended), (1, 1));
        let later = start + Outages::QUIET;
        assert_eq!(outages.begin("resources.example", later), Outage::Started);
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
