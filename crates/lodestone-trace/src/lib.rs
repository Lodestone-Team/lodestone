//! Local debug traces: a topic bus that Lodestone Core and Lodestone Launcher emit
//! events on, and that their command-line tools subscribe to.
//!
//! This is for debugging on the machine itself. Events live in the program's memory:
//! a short backlog, and whatever subscribers the program lets in (the launcher's
//! socket only answers its own user; the core's endpoint only its owner). Nothing is
//! written to disk or sent anywhere by this crate.
//!
//! - **Topics** are dotted names, `ui.click`, `nav.page`, `http.request`. A subscriber
//!   gives patterns ([`Filter`]): `ui` or `ui.*` is everything under `ui`, `*` one
//!   segment (`*.error`), `*` alone everything.
//! - **Events** carry a sequence number, the time and a JSON payload ([`Event`]).
//! - **The backlog** keeps the last events (2000 by default) so a subscriber that
//!   attaches late sees what just happened. Topics marked live-only (busy ones, such as
//!   every game log line or frame time) skip it and cost nothing without a subscriber.
//! - **Secrets** never get in: payloads go through [`scrub`], which blanks fields named
//!   like secrets (password, token, cookie, authorization, API key…) and strings that
//!   look like tokens. Callers also leave them out at the source.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Map, Value};

/// The backlog's size when none is given.
pub const DEFAULT_BACKLOG: usize = 2000;

/// What replaces a secret.
pub const REDACTED: &str = "[redacted]";

/// One event.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Counts up from 1 for the bus's lifetime.
    pub seq: u64,
    /// Unix time in milliseconds.
    pub at_ms: u64,
    pub topic: String,
    pub data: Value,
}

impl Event {
    pub fn to_json(&self) -> Value {
        json!({ "seq": self.seq, "at_ms": self.at_ms, "topic": self.topic, "data": self.data })
    }
}

/// Topic patterns a subscriber wants. Empty wants everything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    patterns: Vec<Vec<String>>,
}

impl Filter {
    /// Everything.
    pub fn all() -> Self {
        Self::default()
    }

    pub fn new<S: AsRef<str>>(patterns: impl IntoIterator<Item = S>) -> Self {
        let patterns = patterns
            .into_iter()
            .map(|p| p.as_ref().trim().to_string())
            .filter(|p| !p.is_empty())
            .map(|p| p.split('.').map(str::to_string).collect())
            .collect();
        Self { patterns }
    }

    pub fn matches(&self, topic: &str) -> bool {
        if self.patterns.is_empty() {
            return true;
        }
        let topic: Vec<&str> = topic.split('.').collect();
        self.patterns.iter().any(|pattern| matches(pattern, &topic))
    }
}

/// `pattern` against `topic`, by segment: a pattern that ends early is a prefix (so `ui`
/// takes `ui.click`), and `*` is one segment, or the rest when it is last.
fn matches(pattern: &[String], topic: &[&str]) -> bool {
    for (index, part) in pattern.iter().enumerate() {
        let last = index + 1 == pattern.len();
        match topic.get(index) {
            None => return false,
            Some(_) if part == "*" && last => return true,
            Some(_) if part == "*" => {}
            Some(segment) if segment == part => {}
            Some(_) => return false,
        }
    }
    true
}

/// Where a subscription's events go. Returns false once the subscriber is gone, which
/// ends the subscription. Called with the bus locked: hand the event on, don't block.
pub type Sink = Box<dyn FnMut(&Event) -> bool + Send>;

struct Subscriber {
    id: u64,
    filter: Filter,
    sink: Sink,
}

struct Inner {
    backlog: VecDeque<Arc<Event>>,
    subscribers: Vec<Subscriber>,
    next_subscriber: u64,
    /// How many events each topic had, for `topics`.
    counts: BTreeMap<String, u64>,
    /// What the program says it is doing now (page, selection, …), for snapshots.
    state: Map<String, Value>,
}

/// The bus. One per program, usually through a `static`.
pub struct TraceBus {
    capacity: usize,
    /// Topic patterns that skip the backlog: emitted only while someone listens.
    live_only: Filter,
    seq: AtomicU64,
    subscriber_count: AtomicUsize,
    inner: Mutex<Inner>,
}

impl TraceBus {
    pub fn new(capacity: usize, live_only: &[&str]) -> Self {
        Self {
            capacity,
            live_only: Filter::new(live_only.iter().copied()),
            seq: AtomicU64::new(0),
            subscriber_count: AtomicUsize::new(0),
            inner: Mutex::new(Inner {
                backlog: VecDeque::new(),
                subscribers: Vec::new(),
                next_subscriber: 1,
                counts: BTreeMap::new(),
                state: Map::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether an event on `topic` would go anywhere: the backlog, or a subscriber.
    /// Cheap when the topic is live-only and nobody listens.
    pub fn wants(&self, topic: &str) -> bool {
        if !self.live_only.matches(topic) || self.live_only.patterns.is_empty() {
            return true;
        }
        if self.subscriber_count.load(Ordering::Relaxed) == 0 {
            return false;
        }
        self.lock()
            .subscribers
            .iter()
            .any(|s| s.filter.matches(topic))
    }

    /// Emits an event. `data` is only made when the event goes somewhere, and goes
    /// through [`scrub`] first.
    pub fn emit(&self, topic: &str, data: impl FnOnce() -> Value) {
        if !self.wants(topic) {
            return;
        }
        let mut data = data();
        scrub(&mut data);
        let event = Arc::new(Event {
            seq: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
            at_ms: now_ms(),
            topic: topic.to_string(),
            data,
        });
        let backlogged = self.live_only.patterns.is_empty() || !self.live_only.matches(topic);
        let mut inner = self.lock();
        *inner.counts.entry(event.topic.clone()).or_default() += 1;
        if backlogged && self.capacity > 0 {
            inner.backlog.push_back(event.clone());
            while inner.backlog.len() > self.capacity {
                inner.backlog.pop_front();
            }
        }
        let before = inner.subscribers.len();
        inner
            .subscribers
            .retain_mut(|s| !s.filter.matches(&event.topic) || (s.sink)(&event));
        let gone = before - inner.subscribers.len();
        if gone > 0 {
            self.subscriber_count.fetch_sub(gone, Ordering::Relaxed);
        }
    }

    /// Subscribes `sink` to the topics `filter` takes. First it gets the backlog's
    /// matching events: the last `backlog` of them, and only those after `since` when
    /// given. Returns the subscription's id.
    pub fn subscribe(
        &self,
        filter: Filter,
        backlog: usize,
        since: Option<u64>,
        mut sink: Sink,
    ) -> u64 {
        let mut inner = self.lock();
        let earlier: Vec<Arc<Event>> = inner
            .backlog
            .iter()
            .filter(|e| since.is_none_or(|since| e.seq > since) && filter.matches(&e.topic))
            .cloned()
            .collect();
        let skip = earlier.len().saturating_sub(backlog);
        for event in &earlier[skip..] {
            if !sink(event) {
                return 0;
            }
        }
        let id = inner.next_subscriber;
        inner.next_subscriber += 1;
        inner.subscribers.push(Subscriber { id, filter, sink });
        self.subscriber_count.fetch_add(1, Ordering::Relaxed);
        id
    }

    /// Ends a subscription. Returns whether it was there.
    pub fn unsubscribe(&self, id: u64) -> bool {
        let mut inner = self.lock();
        let before = inner.subscribers.len();
        inner.subscribers.retain(|s| s.id != id);
        let gone = before - inner.subscribers.len();
        if gone > 0 {
            self.subscriber_count.fetch_sub(gone, Ordering::Relaxed);
        }
        gone > 0
    }

    /// How many subscriptions are open.
    pub fn subscribers(&self) -> usize {
        self.subscriber_count.load(Ordering::Relaxed)
    }

    /// The topics seen so far, with how many events each had.
    pub fn topics(&self) -> Vec<(String, u64)> {
        self.lock()
            .counts
            .iter()
            .map(|(topic, count)| (topic.clone(), *count))
            .collect()
    }

    /// The last sequence number given out.
    pub fn last_seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    /// Records what the program is doing now under `key`, for [`Self::state`]; `null`
    /// removes it. Scrubbed like events.
    pub fn set_state(&self, key: &str, mut value: Value) {
        scrub(&mut value);
        let mut inner = self.lock();
        if value.is_null() {
            inner.state.remove(key);
        } else {
            inner.state.insert(key.to_string(), value);
        }
    }

    /// Everything recorded with [`Self::set_state`].
    pub fn state(&self) -> Value {
        Value::Object(self.lock().state.clone())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Field names whose values are always blanked.
const SECRET_NAMES: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "authorization",
    "cookie",
    "api_key",
    "apikey",
    "credential",
    "private_key",
    "jwt",
];

/// Whether a field called `name` holds a secret.
pub fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    SECRET_NAMES.iter().any(|secret| name.contains(secret))
}

/// Blanks secrets in `value`, in place: fields named like secrets, strings that look
/// like tokens, and secret query parameters in URLs.
pub fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if is_secret_name(key) && !item.is_null() && !item.is_boolean() {
                    *item = Value::String(REDACTED.into());
                } else {
                    scrub(item);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub),
        Value::String(text) => {
            if let Some(clean) = scrub_str(text) {
                *text = clean;
            }
        }
        _ => {}
    }
}

/// `text` without the secrets in it, or `None` when it has none.
pub fn scrub_str(text: &str) -> Option<String> {
    if looks_like_token(text.trim()) {
        return Some(REDACTED.into());
    }
    let mut changed = false;
    let words: Vec<String> = text
        .split(' ')
        .scan(false, |after_scheme, word| {
            let out = if (*after_scheme && !word.is_empty()) || looks_like_token(word) {
                changed = true;
                REDACTED.to_string()
            } else if let Some(clean) = scrub_query(word) {
                changed = true;
                clean
            } else {
                word.to_string()
            };
            // "Bearer xyz", "Basic xyz": the word after is the secret
            *after_scheme = matches!(word.to_ascii_lowercase().as_str(), "bearer" | "basic");
            Some(out)
        })
        .collect();
    changed.then(|| words.join(" "))
}

/// A URL (or path) with secret query parameters blanked, or `None` when it has none.
fn scrub_query(word: &str) -> Option<String> {
    let (base, query) = word.split_once('?')?;
    let mut changed = false;
    let parts: Vec<String> = query
        .split('&')
        .map(|part| match part.split_once('=') {
            Some((name, value))
                if !value.is_empty()
                    && (is_secret_name(name) || name == "code" || name == "key") =>
            {
                changed = true;
                format!("{name}={REDACTED}")
            }
            _ => part.to_string(),
        })
        .collect();
    changed.then(|| format!("{base}?{}", parts.join("&")))
}

/// Whether `word` looks like a token: a JWT, a Lodestone API key (`lsk_…`), a
/// Microsoft token, or a long run of mixed letters and digits with no path in it.
pub fn looks_like_token(word: &str) -> bool {
    let word = word.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')'));
    if word.len() < 20 || word.chars().any(char::is_whitespace) {
        return false;
    }
    if word.starts_with("lsk_") || word.starts_with("M.C") || word.starts_with("M.R3_") {
        return true;
    }
    let jwt = word.starts_with("eyJ") && word.matches('.').count() == 2;
    if jwt {
        return true;
    }
    if word.len() < 40 || word.contains('/') || word.contains("://") {
        return false;
    }
    let tokenish = word.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '=' | '+' | '*' | '!' | '~')
    });
    let upper = word.chars().any(|c| c.is_ascii_uppercase());
    let lower = word.chars().any(|c| c.is_ascii_lowercase());
    let digit = word.chars().any(|c| c.is_ascii_digit());
    tokenish && upper && lower && digit
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn collect(bus: &TraceBus, filter: Filter, backlog: usize) -> (u64, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let id = bus.subscribe(
            filter,
            backlog,
            None,
            Box::new(move |e| tx.send(e.clone()).is_ok()),
        );
        (id, rx)
    }

    #[test]
    fn patterns_match_by_segment() {
        let filter = Filter::new(["ui", "job.*", "*.error", "http.request"]);
        for topic in [
            "ui",
            "ui.click",
            "ui.modal.open",
            "job.started",
            "log.error",
            "http.request",
        ] {
            assert!(filter.matches(topic), "{topic}");
        }
        for topic in ["uix", "job", "log.warn", "http.response", "nav.page"] {
            assert!(!filter.matches(topic), "{topic}");
        }
        assert!(Filter::all().matches("anything.at.all"));
        assert!(Filter::new(["*"]).matches("nav.page"));
    }

    #[test]
    fn subscribers_get_the_backlog_then_live_events() {
        let bus = TraceBus::new(3, &[]);
        for n in 0..5 {
            bus.emit("ui.click", || json!({ "n": n }));
        }
        bus.emit("nav.page", || json!({ "page": "servers" }));
        // the backlog keeps the last 3: clicks 3 and 4, and the page
        let (_, rx) = collect(&bus, Filter::new(["ui"]), 10);
        let got: Vec<u64> = rx
            .try_iter()
            .map(|e| e.data["n"].as_u64().unwrap())
            .collect();
        assert_eq!(got, [3, 4]);
        let (id, rx) = collect(&bus, Filter::new(["nav"]), 0);
        assert_eq!(rx.try_iter().count(), 0, "backlog 0 replays nothing");
        bus.emit("nav.tab", || json!({ "tab": "content" }));
        bus.emit("ui.click", || json!({}));
        let live: Vec<String> = rx.try_iter().map(|e| e.topic).collect();
        assert_eq!(live, ["nav.tab"]);
        assert!(bus.unsubscribe(id));
        assert!(!bus.unsubscribe(id));
        assert_eq!(bus.last_seq(), 8);
    }

    #[test]
    fn live_only_topics_cost_nothing_without_a_subscriber() {
        let bus = TraceBus::new(10, &["perf", "game.log"]);
        let mut made = 0;
        bus.emit("perf.frame", || {
            made += 1;
            json!({})
        });
        assert_eq!(made, 0, "nobody listens: the payload isn't made");
        assert!(bus.wants("ui.click"), "other topics go to the backlog");
        let (_, rx) = collect(&bus, Filter::new(["perf"]), 10);
        bus.emit("perf.frame", || json!({ "ms": 3 }));
        assert_eq!(rx.try_iter().count(), 1);
        let (_, late) = collect(&bus, Filter::all(), 10);
        assert_eq!(
            late.try_iter().count(),
            0,
            "live-only events skip the backlog"
        );
    }

    #[test]
    fn a_gone_subscriber_is_dropped() {
        let bus = TraceBus::new(10, &[]);
        let (_, rx) = collect(&bus, Filter::all(), 0);
        drop(rx);
        bus.emit("ui.click", || json!({}));
        assert_eq!(bus.subscribers(), 0);
    }

    #[test]
    fn secrets_are_scrubbed() {
        let mut value = json!({
            "username": "steve",
            "password": "hunter2",
            "Authorization": "Bearer abc",
            "refresh_token": "M.R3_BAY.abcdef",
            "has_token": true,
            "nested": [{ "api_key": "x" }, "lsk_0123abcd_ZXhhbXBsZXNlY3JldGtleXZhbHVl"],
            "url": "https://login.live.com/oauth20_desktop.srf?code=M.C123&state=ok",
            "line": "sent Bearer eyJhbGciOi.eyJzdWIiOjF9.c2lnbmF0dXJl to the core",
            "hash": "3f786850e387550fdab836ed7e6dc881de23001b",
            "path": "/Users/steve/Library/Application Support/Lodestone/instances/Survival-1.20.1",
            "uuid": "7c1f3e8a-0b2d-4f6e-9a11-2d3c4b5a6978",
        });
        scrub(&mut value);
        assert_eq!(value["username"], "steve");
        assert_eq!(value["password"], REDACTED);
        assert_eq!(value["Authorization"], REDACTED);
        assert_eq!(value["refresh_token"], REDACTED);
        assert_eq!(value["has_token"], true, "a flag says nothing secret");
        assert_eq!(value["nested"][0]["api_key"], REDACTED);
        assert_eq!(value["nested"][1], REDACTED);
        assert_eq!(
            value["url"],
            "https://login.live.com/oauth20_desktop.srf?code=[redacted]&state=ok"
        );
        assert_eq!(value["line"], "sent Bearer [redacted] to the core");
        assert_eq!(value["hash"], "3f786850e387550fdab836ed7e6dc881de23001b");
        assert!(value["path"].as_str().unwrap().starts_with("/Users/steve"));
        assert_eq!(value["uuid"], "7c1f3e8a-0b2d-4f6e-9a11-2d3c4b5a6978");
    }

    #[test]
    fn events_and_state_are_scrubbed_on_the_way_in() {
        let bus = TraceBus::new(10, &[]);
        bus.emit("http.request", || json!({ "headers": { "cookie": "a=b" } }));
        let (_, rx) = collect(&bus, Filter::all(), 10);
        assert_eq!(rx.recv().unwrap().data["headers"]["cookie"], REDACTED);
        bus.set_state("account", json!({ "name": "steve", "token": "x" }));
        bus.set_state("modal", json!("settings"));
        bus.set_state("modal", Value::Null);
        assert_eq!(
            bus.state(),
            json!({ "account": { "name": "steve", "token": REDACTED } })
        );
        assert_eq!(bus.topics(), [("http.request".to_string(), 1)]);
    }
}
