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
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
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
    /// Off: nothing is recorded or sent, and [`Self::wants`] is false for every topic.
    enabled: AtomicBool,
    inner: Mutex<Inner>,
}

impl TraceBus {
    pub fn new(capacity: usize, live_only: &[&str]) -> Self {
        Self {
            capacity,
            live_only: Filter::new(live_only.iter().copied()),
            seq: AtomicU64::new(0),
            subscriber_count: AtomicUsize::new(0),
            enabled: AtomicBool::new(true),
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
        if !self.enabled.load(Ordering::Relaxed) {
            return false;
        }
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

    /// Turns recording on or off. Off clears the backlog and ends every subscription,
    /// and emitting costs one atomic load.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            let mut inner = self.lock();
            inner.backlog.clear();
            let gone = inner.subscribers.len();
            inner.subscribers.clear();
            self.subscriber_count.fetch_sub(gone, Ordering::Relaxed);
        }
    }

    /// Whether events are recorded.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
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
    "passphrase",
    "jwt",
];

/// Short names that are secrets only on their own (`pass=…`), not inside a longer
/// word (`passed`, `bypass`).
const SECRET_WORDS: &[&str] = &["pass", "pwd"];

/// Whether a field called `name` holds a secret. Case, `-` and `_` don't matter
/// (`X-Api-Key`, `privateKey`, `private_key`).
pub fn is_secret_name(name: &str) -> bool {
    let name: String = name
        .chars()
        .filter(|c| !matches!(c, '-' | '_'))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    SECRET_WORDS.contains(&name.as_str())
        || SECRET_NAMES
            .iter()
            .any(|secret| name.contains(&secret.replace('_', "")))
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
            } else if let Some(clean) = scrub_download_key(word) {
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

/// A core's download link (`…/file/<key>`) with its key blanked: the key is all a
/// download needs.
fn scrub_download_key(word: &str) -> Option<String> {
    let start = word.find("/file/")? + "/file/".len();
    let end = word[start..]
        .find(|c: char| !c.is_ascii_alphanumeric())
        .map_or(word.len(), |n| start + n);
    (end - start >= 16).then(|| format!("{}{REDACTED}{}", &word[..start], &word[end..]))
}

/// A URL (or path) with secret query parameters blanked, or `None` when it has none.
/// A parameter is secret by its name (a secret's, `code`, `key`, or an `invite` or
/// `link`, which carry invite links), or by its value once percent-decoded: a filter
/// like `{"bearer_token":"lsk_…"}` goes whole.
fn scrub_query(word: &str) -> Option<String> {
    let (base, query) = word.split_once('?')?;
    let mut changed = false;
    let parts: Vec<String> = query
        .split('&')
        .map(|part| match part.split_once('=') {
            Some((name, value))
                if !value.is_empty()
                    && !value.starts_with(REDACTED)
                    && (is_secret_name(name)
                        || matches!(name, "code" | "key" | "invite" | "link")
                        || decoded_has_secret(value)) =>
            {
                changed = true;
                // what closes the URL in its sentence stays: `(…?token=x)`
                let end = value
                    .find([')', ']', '}', '>', '"', '\'', ',', ';'])
                    .unwrap_or(value.len());
                format!("{name}={REDACTED}{}", &value[end..])
            }
            _ => part.to_string(),
        })
        .collect();
    changed.then(|| format!("{base}?{}", parts.join("&")))
}

/// Whether a query value holds a secret once percent-decoded.
fn decoded_has_secret(value: &str) -> bool {
    let decoded = percent_decode(value);
    let text = decoded.as_deref().unwrap_or(value);
    scrub_named_values(text).is_some()
        || scrub_token_runs(text).is_some()
        || scrub_links(text).is_some()
}

/// `text` with its `%XX` escapes decoded, or `None` when it has none.
fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[at], bytes.get(at + 1), bytes.get(at + 2)) {
            (b'%', Some(&high), Some(&low)) if hex(high).is_some() && hex(low).is_some() => {
                out.push((hex(high).unwrap_or(0) * 16 + hex(low).unwrap_or(0)) as u8);
                at += 3;
            }
            (b, _, _) => {
                out.push(b);
                at += 1;
            }
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// A line of text for a log file without the secrets in it:
///
/// - everything after `Authorization:` on its line, and every cookie after `Cookie:`;
/// - secret query parameters ([`scrub_query`]), a URL's password
///   (`https://user:[redacted]@host`) and the part after `#` of invite and friend links
///   (`…/j/…#…`, `…/f/…#…`, `lhp2://…`);
/// - the value after a name like a secret's ([`is_secret_name`]): `name=value`,
///   `name: value`, `name = "two words"`, JSON's `"name": "value"` (escaped too), a
///   pair `("authorization", "Bearer x")`, a `--flag value` (`--accessToken …` on the
///   game's command line), and `code=`/`key=` unless the value is a number (an exit
///   code says nothing secret). A secret name's value goes whatever it is (a number
///   too, as [`scrub`] blanks it); only `true`, `false`, `null` and `None` stay;
/// - the credential after `Bearer` (and `Basic`, when it looks like one);
/// - tokens ([`looks_like_token`]) anywhere, glued to punctuation or not:
///   `Some("eyJ…")`, long base64 runs with `+` and `/` in them (Xbox tokens), and a
///   core's download key after `/file/`.
///
/// Multi-line text is scrubbed line by line. Text without secrets comes back borrowed,
/// and scrubbed text comes back the same: scrubbing twice changes nothing. This is a
/// heuristic, a last line of defence: callers leave secrets out at the source.
pub fn scrub_line(text: &str) -> std::borrow::Cow<'_, str> {
    let mut out = String::new();
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        let (body, end) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        match scrub_one_line(body) {
            Some(clean) => {
                changed = true;
                out.push_str(&clean);
            }
            None => out.push_str(body),
        }
        out.push_str(end);
    }
    if changed {
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// [`scrub_line`] for one line, or `None` when it has no secrets.
fn scrub_one_line(line: &str) -> Option<String> {
    const PASSES: [fn(&str) -> Option<String>; 8] = [
        scrub_authorization,
        scrub_queries,
        scrub_userinfo,
        scrub_links,
        scrub_named_values,
        scrub_schemes,
        scrub_base64_runs,
        scrub_token_runs,
    ];
    let mut line = std::borrow::Cow::Borrowed(line);
    let mut changed = false;
    for pass in PASSES {
        if let Some(clean) = pass(&line) {
            line = std::borrow::Cow::Owned(clean);
            changed = true;
        }
    }
    changed.then(|| line.into_owned())
}

/// Everything after `Authorization:` blanked.
fn scrub_authorization(line: &str) -> Option<String> {
    const AUTHORIZATION: &str = "authorization:";
    let at = line.to_ascii_lowercase().find(AUTHORIZATION)?;
    let end = at + AUTHORIZATION.len();
    let rest = line[end..].trim();
    (!rest.is_empty() && rest != REDACTED).then(|| format!("{} {REDACTED}", &line[..end]))
}

/// Each space-separated word's secret query parameters blanked.
fn scrub_queries(line: &str) -> Option<String> {
    map_words(line, scrub_query)
}

/// A URL's password blanked, keeping the user name: `https://steve:[redacted]@host`.
fn scrub_userinfo(line: &str) -> Option<String> {
    map_words(line, |word| {
        let start = word.find("://")? + 3;
        let authority_end = word[start..]
            .find(['/', '?', '#', '"', '\'', ')', ']'])
            .map_or(word.len(), |n| start + n);
        let at = start + word[start..authority_end].rfind('@')?;
        let colon = start + word[start..at].find(':')?;
        let password = &word[colon + 1..at];
        (!password.is_empty() && password != REDACTED)
            .then(|| format!("{}{REDACTED}{}", &word[..colon + 1], &word[at..]))
    })
}

/// The secret part of invite and friend links blanked: what follows `#` in a link
/// whose path is `/j/…` or `/f/…`, or in an `lhp2://` link.
fn scrub_links(line: &str) -> Option<String> {
    map_words(line, |word| {
        let (base, fragment) = word.split_once('#')?;
        let lower = base.to_ascii_lowercase();
        let link = lower.contains("lhp2://")
            || (lower.contains("://") && (lower.contains("/j/") || lower.contains("/f/")));
        (link && !fragment.is_empty() && !fragment.starts_with(REDACTED))
            .then(|| format!("{base}#{REDACTED}"))
    })
}

/// `line` with each space-separated word that `f` changes replaced.
fn map_words(line: &str, f: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut changed = false;
    let words: Vec<String> = line
        .split(' ')
        .map(|word| match f(word) {
            Some(clean) => {
                changed = true;
                clean
            }
            None => word.to_string(),
        })
        .collect();
    changed.then(|| words.join(" "))
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// The values after names like secrets' blanked (see [`scrub_line`]). It finds each
/// name on its own, so a stray quote earlier in the line changes nothing.
fn scrub_named_values(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let skip_spaces = |mut at: usize| {
        while at < len && matches!(bytes[at], b' ' | b'\t') {
            at += 1;
        }
        at
    };
    let mut out = String::new();
    let mut copied = 0;
    let mut at = 0;
    while at < len {
        if !is_name_byte(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < len && is_name_byte(bytes[at]) {
            at += 1;
        }
        let run = &line[start..at];
        let flag = run.starts_with('-');
        let name = run.trim_start_matches('-');
        let numbered = matches!(name, "code" | "key");
        if name.is_empty() || !(is_secret_name(name) || numbered) {
            continue;
        }
        let quoted = start > 0 && matches!(bytes[start - 1], b'"' | b'\'');
        // the name's closing quote, `"` or `\"` (JSON inside a Debug string)
        let mut next = at;
        if quoted {
            if bytes.get(next) == Some(&b'\\') && bytes.get(next + 1) == Some(&b'"') {
                next += 2;
            } else if matches!(bytes.get(next), Some(b'"' | b'\'')) {
                next += 1;
            }
        }
        let before_separator = next;
        next = skip_spaces(next);
        let separator = bytes.get(next).copied();
        let separated = match separator {
            // not `::` (a Rust path) or `==`
            Some(b'=' | b':') if bytes.get(next + 1) != separator.as_ref() => {
                next += 1;
                true
            }
            // a pair: `("authorization", "Bearer x")`, `["--accessToken", "x"]`
            Some(b',') if quoted => {
                next += 1;
                true
            }
            // `--accessToken x`
            _ => flag && next > before_separator,
        };
        // `code` and `key` count only as `code=…`, outside JSON
        if !separated || (numbered && !is_secret_name(name) && (quoted || separator != Some(b'=')))
        {
            continue;
        }
        let value_start = skip_spaces(next);
        let Some((value_end, inner, quote)) = value_at(line, value_start) else {
            continue;
        };
        let plain = inner.is_empty()
            || inner.starts_with(REDACTED)
            || matches!(inner, "true" | "false" | "null" | "None")
            || (quote.is_empty()
                && inner.chars().all(|c| c.is_ascii_digit())
                && numbered
                && !is_secret_name(name));
        if plain || (separator == Some(b',') && quote.is_empty()) {
            continue;
        }
        // `Cookie: a=1; session=abc`: every cookie on the line
        if !quoted && separator == Some(b':') && name.to_ascii_lowercase().ends_with("cookie") {
            out.push_str(&line[copied..value_start]);
            out.push_str(REDACTED);
            copied = len;
            break;
        }
        // `authorization=Bearer abc`: the credential too
        let value_end = if quote.is_empty()
            && matches!(inner.to_ascii_lowercase().as_str(), "bearer" | "basic")
        {
            value_at(line, skip_spaces(value_end)).map_or(value_end, |(end, _, _)| end)
        } else {
            value_end
        };
        out.push_str(&line[copied..value_start]);
        out.push_str(quote);
        out.push_str(REDACTED);
        out.push_str(quote);
        copied = value_end;
        at = value_end;
    }
    (copied > 0).then(|| {
        out.push_str(&line[copied..]);
        out
    })
}

/// The value starting at `start`: where it ends, what is inside its quotes, and the
/// quote (`"`, `'`, `\"`, or none for a bare word). A quoted value runs to its closing
/// quote (or the line's end), a bare one to a space or `,;)]}&"`.
fn value_at(line: &str, start: usize) -> Option<(usize, &str, &'static str)> {
    let bytes = line.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    let quote: &'static str = match bytes[start] {
        b'\\' if bytes.get(start + 1) == Some(&b'"') => "\\\"",
        b'"' => "\"",
        b'\'' => "'",
        _ => "",
    };
    if quote.is_empty() {
        if line[start..].starts_with(REDACTED) {
            let end = start + REDACTED.len();
            return Some((end, &line[start..end], quote));
        }
        let end = line[start..]
            .find([' ', '\t', ',', ';', ')', ']', '}', '&', '"'])
            .map_or(line.len(), |n| start + n);
        return (end > start).then(|| (end, &line[start..end], quote));
    }
    let open = start + quote.len();
    let mut at = open;
    while at < bytes.len() {
        if bytes[at..].starts_with(quote.as_bytes()) {
            return Some((at + quote.len(), &line[open..at], quote));
        }
        // an escaped quote inside a `"` string doesn't end it
        at += if quote == "\"" && bytes[at] == b'\\' {
            2
        } else {
            1
        };
    }
    let at = line.len();
    Some((at, &line[open.min(at)..at], quote))
}

/// The credential after `Bearer` blanked, and after `Basic` when it looks like one
/// (mixed-case base64, so `Basic settings` stays).
fn scrub_schemes(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let bytes = line.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        let scheme = ["bearer", "basic"]
            .into_iter()
            .find(|scheme| lower.as_bytes()[at..].starts_with(scheme.as_bytes()));
        let Some(scheme) = scheme else {
            at += 1;
            continue;
        };
        let end = at + scheme.len();
        let whole = (at == 0 || !bytes[at - 1].is_ascii_alphanumeric())
            && matches!(bytes.get(end), Some(b' ' | b'\t'));
        if !whole || at < copied {
            at = end;
            continue;
        }
        let value_start = (end..bytes.len())
            .find(|&n| !matches!(bytes[n], b' ' | b'\t'))
            .unwrap_or(bytes.len());
        // already blanked (this is the second time the line is scrubbed)
        if line[value_start..].starts_with(REDACTED) {
            at = value_start + REDACTED.len();
            continue;
        }
        let value_end = line[value_start..]
            .find([' ', '\t', '"', '\'', ',', ';', ')', ']', '}'])
            .map_or(line.len(), |n| value_start + n);
        let value = &line[value_start..value_end];
        let credential = !value.is_empty()
            && !value.starts_with(REDACTED)
            && (scheme == "bearer"
                || (value.len() >= 8
                    && value
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
                    && value.chars().any(|c| c.is_ascii_uppercase())
                    && value.chars().any(|c| c.is_ascii_lowercase())));
        if credential {
            out.push_str(&line[copied..value_start]);
            out.push_str(REDACTED);
            copied = value_end;
        }
        at = value_end.max(end);
    }
    (copied > 0).then(|| {
        out.push_str(&line[copied..]);
        out
    })
}

/// Long base64 runs blanked, `/` and all: an Xbox Live token (`EwAoA+pv…/…==`) has
/// `/` in it, which [`scrub_token_runs`] reads as a path. A run counts when it is at
/// least 40 characters, has `+` in it (a path rarely does) and mixed letters and
/// digits, and no `.` or `:` touches it (a URL or a file name).
fn scrub_base64_runs(line: &str) -> Option<String> {
    let is_base64 = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=');
    let bytes = line.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        if !is_base64(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && is_base64(bytes[at]) {
            at += 1;
        }
        let run = &line[start..at];
        let touches = |b: Option<&u8>| matches!(b, Some(b'.' | b':'));
        let base64 = run.len() >= 40
            && run.contains('+')
            && run.bytes().any(|b| b.is_ascii_uppercase())
            && run.bytes().any(|b| b.is_ascii_lowercase())
            && run.bytes().any(|b| b.is_ascii_digit())
            && !touches(start.checked_sub(1).and_then(|n| bytes.get(n)))
            && !touches(bytes.get(at));
        if base64 {
            out.push_str(&line[copied..start]);
            out.push_str(REDACTED);
            copied = at;
        }
    }
    (copied > 0).then(|| {
        out.push_str(&line[copied..]);
        out
    })
}

/// Characters a token is made of.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '*' | '!' | '~' | '$')
}

/// Tokens blanked wherever they are: each run of token characters is checked on its
/// own, so `Some("eyJ…")` and `["--accessToken","eyJ…"]` lose theirs. An `lsk_` key
/// inside a run (`%22lsk_…`) goes from where it starts. A path's part is only blanked
/// for a known token shape, so a long file name stays.
fn scrub_token_runs(line: &str) -> Option<String> {
    let mut out = String::new();
    let mut copied = 0;
    let mut chars = line.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if !is_token_char(c) {
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(at, c)) = chars.peek() {
            if !is_token_char(c) {
                break;
            }
            end = at + c.len_utf8();
            chars.next();
        }
        // a sentence's full stop isn't part of a token
        let run = line[start..end].trim_matches('.');
        let start = start + line[start..end].find(run).unwrap_or(0);
        let end = start + run.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        let in_path = matches!(before, Some('/' | '\\')) || matches!(after, Some('/' | '\\'));
        // a core's download link, `/api/v1/file/<key>`: the key is all it takes
        let download_key = line[..start].ends_with("/file/")
            && run.len() >= 16
            && run.chars().all(|c| c.is_ascii_alphanumeric());
        let from = if let Some(key) = run.find("lsk_").filter(|&n| run.len() - n >= 20) {
            Some(start + key)
        } else if download_key || is_known_token(run) || (!in_path && looks_like_token(run)) {
            Some(start)
        } else {
            None
        };
        if let Some(from) = from {
            out.push_str(&line[copied..from]);
            out.push_str(REDACTED);
            copied = end;
        }
    }
    (copied > 0).then(|| {
        out.push_str(&line[copied..]);
        out
    })
}

/// A token of a known shape: a JWT, a Lodestone API key (`lsk_…`) or a Microsoft
/// token.
fn is_known_token(word: &str) -> bool {
    let word = word.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')'));
    if word.len() < 20 || word.chars().any(char::is_whitespace) {
        return false;
    }
    word.starts_with("lsk_")
        || word.starts_with("M.C")
        || word.starts_with("M.R3_")
        || (word.starts_with("eyJ") && word.matches('.').count() == 2)
}

/// Whether `word` looks like a token: a JWT, a Lodestone API key (`lsk_…`), a
/// Microsoft token, or a long run of mixed letters and digits with no path in it.
pub fn looks_like_token(word: &str) -> bool {
    if is_known_token(word) {
        return true;
    }
    let word = word.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')'));
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
    fn a_disabled_bus_records_nothing() {
        let bus = TraceBus::new(10, &["game.log"]);
        bus.emit("ui.click", || json!({ "n": 1 }));
        let (_id, rx) = collect(&bus, Filter::all(), 0);
        bus.set_enabled(false);
        assert!(!bus.is_enabled());
        assert_eq!(bus.subscribers(), 0);
        assert!(!bus.wants("ui.click"));
        let mut made = false;
        bus.emit("ui.click", || {
            made = true;
            json!({})
        });
        assert!(!made, "the event's data was made");
        assert_eq!(bus.last_seq(), 1);
        assert!(rx.try_recv().is_err());

        bus.set_enabled(true);
        let (_id, rx) = collect(&bus, Filter::all(), 10);
        assert!(rx.try_recv().is_err(), "the backlog was kept");
        bus.emit("ui.click", || json!({ "n": 2 }));
        assert_eq!(rx.try_recv().unwrap().data, json!({ "n": 2 }));
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
    fn download_keys_are_scrubbed() {
        let mut value = json!({
            "path": "/api/v1/file/Ab3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE",
            "other": "/api/v1/file/short",
        });
        scrub(&mut value);
        assert_eq!(value["path"], "/api/v1/file/[redacted]");
        assert_eq!(value["other"], "/api/v1/file/short");
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

    #[test]
    fn log_lines_lose_their_secrets() {
        let cases = [
            ("refresh failed: password=hunter2 user=steve", "refresh failed: password=[redacted] user=steve"),
            ("got access_token: abc123 for steve", "got access_token: [redacted] for steve"),
            (
                r#"body {"access_token":"abc","expires_in":3600,"has_token":true}"#,
                r#"body {"access_token":"[redacted]","expires_in":3600,"has_token":true}"#,
            ),
            (
                r#"{"name": "steve", "refresh_token": "M.R3_x"}"#,
                r#"{"name": "steve", "refresh_token": "[redacted]"}"#,
            ),
            (
                "java -Xmx2G net.minecraft.client.main.Main --username steve --accessToken abc.def --version 1.21",
                "java -Xmx2G net.minecraft.client.main.Main --username steve --accessToken [redacted] --version 1.21",
            ),
            ("request headers Authorization: Bearer abc", "request headers Authorization: [redacted]"),
            ("authorization: Basic dXNlcjpwYXNz", "authorization: [redacted]"),
            ("sent Bearer xyz to the core", "sent Bearer [redacted] to the core"),
            (
                "key lsk_0123abcd_ZXhhbXBsZXNlY3JldGtleXZhbHVl refused",
                "key [redacted] refused",
            ),
            ("code M.C507_BAY.2.U.abcdef0123456789 expired", "code [redacted] expired"),
            (
                "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOjF9.c2lnbmF0dXJlc2ln end",
                "jwt [redacted] end",
            ),
            (
                "GET https://core.example/api?key=abc&x=1 failed",
                "GET https://core.example/api?key=[redacted]&x=1 failed",
            ),
            ("Token { access_token: \"abc\" }", "Token { access_token: \"[redacted]\" }"),
            ("sign-in code=AbCd1234 refused", "sign-in code=[redacted] refused"),
            (
                "url https://alice:hunter2pass@example.com/path",
                "url https://alice:[redacted]@example.com/path",
            ),
            (
                "connecting to https://alice:hunter2@example.com/path",
                "connecting to https://alice:[redacted]@example.com/path",
            ),
            (
                "error sending request for url (https://api.minecraftservices.com/minecraft/profile?access_token=abcdef12345)",
                "error sending request for url (https://api.minecraftservices.com/minecraft/profile?access_token=[redacted])",
            ),
            ("Cookie: a=1; session=abcdef123456", "Cookie: [redacted]"),
            (r#"{"privateKey": "abc-short"}"#, r#"{"privateKey": "[redacted]"}"#),
            (
                "pass=xyz pwd=abc passphrase=hunter2",
                "pass=[redacted] pwd=[redacted] passphrase=[redacted]",
            ),
            ("token\t= x ok", "token\t= [redacted] ok"),
            ("sent Bearer\tabc ok", "sent Bearer\t[redacted] ok"),
            (
                "xbl EwAoA+pvBAAUKods63Ys1fGlwiccIFJ+qE1hANsAAZ/abcDEF123+ghiJKL456/xyzXYZ789== ok",
                "xbl [redacted] ok",
            ),
            (
                r#"{"token": 123456, "password": 98765, "code": 7}"#,
                r#"{"token": [redacted], "password": [redacted], "code": 7}"#,
            ),
            (
                "error sending request for url (https://core.example:16662/api/v1/file/Ab3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE): client error (Connect)",
                "error sending request for url (https://core.example:16662/api/v1/file/[redacted]): client error (Connect)",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(scrub_line(input), expected, "{input}");
        }
    }

    /// Every line the other tests scrub, for [`scrubbing_twice_changes_nothing`].
    const PROBES: &[&str] = &[
        "refresh failed: password=hunter2 user=steve",
        "got access_token: abc123 for steve",
        r#"body {"access_token":"abc","expires_in":3600,"has_token":true}"#,
        "java -Xmx2G Main --username steve --accessToken abc.def --version 1.21",
        "request headers Authorization: Bearer abc",
        "sent Bearer xyz to the core",
        "x Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOjF9.c2lnbmF0dXJlc2ln",
        "header \"Bearer abc\"",
        "authorization=Bearer abc ok",
        r#"("authorization", "Bearer abc")"#,
        "key lsk_0123abcd_ZXhhbXBsZXNlY3JldGtleXZhbHVl refused",
        "GET https://core.example/api?key=abc&x=1 failed",
        "(https://api.example/profile?access_token=abcdef12345)",
        "url https://alice:hunter2@example.com/path",
        "Cookie: a=1; session=abcdef123456",
        "Token { access_token: \"abc\" }",
        r#"body "{\"refresh_token\":\"abc\",\"n\":1}""#,
        r#"password="hunter two" ok"#,
        "xbl EwAoA+pvBAAUKods63Ys1fGlwiccIFJ+qE1hANsAAZ/abcDEF123+ghiJKL456/xyzXYZ789==",
        "joining https://connect.lodestone.cc/j/ABCDEFGH#0123456789abcdef0123 failed",
        "GET /api/v1/file/Ab3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE",
        r#"{"token": 123456}"#,
    ];

    #[test]
    fn scrubbing_twice_changes_nothing() {
        for probe in PROBES {
            let once = scrub_line(probe).into_owned();
            assert_ne!(once, *probe, "{probe} has a secret");
            assert_eq!(scrub_line(&once), once, "{probe}");
        }
    }

    #[test]
    fn secrets_glued_to_punctuation_or_after_stray_quotes_go_too() {
        const JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOjF9.c2lnbmF0dXJlc2ln";
        let cases = [
            (
                format!("Some(\"{JWT}\")"),
                "Some(\"[redacted]\")".to_string(),
            ),
            (
                format!("[\"--accessToken\",\"{JWT}\"]"),
                "[\"--accessToken\",\"[redacted]\"]".to_string(),
            ),
            (
                r#"args ["--accessToken", "abcDEF123xyz"]"#.to_string(),
                r#"args ["--accessToken", "[redacted]"]"#.to_string(),
            ),
            (
                format!("parse \"{{\"access_token\":\"{JWT}\"}}"),
                "parse \"{\"access_token\":\"[redacted]\"}".to_string(),
            ),
            (
                format!("body 5\" screen {{\"access_token\":\"{JWT}\"}}"),
                "body 5\" screen {\"access_token\":\"[redacted]\"}".to_string(),
            ),
            (
                r#"he said " then {"password":"hunter2"}"#.to_string(),
                r#"he said " then {"password":"[redacted]"}"#.to_string(),
            ),
            (
                format!("Err value: Reply {{ session: Some(\"{JWT}\") }}"),
                "Err value: Reply { session: Some(\"[redacted]\") }".to_string(),
            ),
            (
                r#"body "{\"refresh_token\":\"abc\",\"n\":1}""#.to_string(),
                r#"body "{\"refresh_token\":\"[redacted]\",\"n\":1}""#.to_string(),
            ),
            (
                "password = hunter2 ok".into(),
                "password = [redacted] ok".into(),
            ),
            (
                r#"password="hunter two" ok"#.into(),
                r#"password="[redacted]" ok"#.into(),
            ),
            (
                r#"access_token: "abc def" ok"#.into(),
                r#"access_token: "[redacted]" ok"#.into(),
            ),
            (
                "authorization=Bearer abc ok".into(),
                "authorization=[redacted] ok".into(),
            ),
            (
                "X-Api-Key: abc ok".into(),
                "X-Api-Key: [redacted] ok".into(),
            ),
            (
                r#"("authorization", "Bearer abc")"#.into(),
                r#"("authorization", "[redacted]")"#.into(),
            ),
            (
                r#"header "Bearer abc""#.into(),
                r#"header "Bearer [redacted]""#.into(),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(scrub_line(&input), expected, "{input}");
        }
    }

    #[test]
    fn encoded_and_linked_secrets_go() {
        // the Events stream's URL, as tungstenite's trace prints its handshake
        let key = "lsk_0a1b2c3d_ZXhhbXBsZXNlY3JldGtleXZhbHVl";
        let line = format!(
            "Request: \"GET /api/v1/events/all/stream?filter=%7B%22bearer_token%22%3A%22{key}%22%2C%22event_types%22%3A%5B%5D%7D HTTP/1.1\\r\\nHost: 127.0.0.1:16662\\r\\n\""
        );
        let clean = scrub_line(&line);
        assert!(
            !clean.contains("lsk_") && !clean.contains("ZXhh"),
            "{clean}"
        );
        assert!(
            clean.contains("stream?filter=[redacted] HTTP/1.1"),
            "{clean}"
        );
        // invite and friend links
        let hex = "0123456789abcdef".repeat(4);
        for (input, expected) in [
            (
                format!("joining https://connect.lodestone.cc/j/ABCDEFGH#{hex}.{} failed", &hex[..32]),
                "joining https://connect.lodestone.cc/j/ABCDEFGH#[redacted] failed".to_string(),
            ),
            (
                format!("opened lodestone://join?invite=https%3A%2F%2Fconnect.lodestone.cc%2Fj%2FAB%23{hex}"),
                "opened lodestone://join?invite=[redacted]".to_string(),
            ),
            (
                format!("friend https://connect.lodestone.cc/f/xyz#{hex} added"),
                "friend https://connect.lodestone.cc/f/xyz#[redacted] added".to_string(),
            ),
            (format!("lhp2://peer#{hex}"), "lhp2://peer#[redacted]".to_string()),
        ] {
            assert_eq!(scrub_line(&input), expected, "{input}");
        }
    }

    #[test]
    fn hostile_text_never_panics() {
        for text in [
            "é\"password\":\"ü",
            "password=\"unterminated é",
            "\\\"token\\\":\\\"",
            "Bearer",
            "Basic é",
            "?=&=?%%%zz%e9",
            "--token",
            "lsk_",
            "\"",
            "token: é",
        ] {
            let _ = scrub_line(text);
        }
    }

    #[test]
    fn log_lines_keep_plain_text() {
        for plain in [
            "the core exited with code=1 after 3 s",
            "saved 12 files to /Users/steve/Library/Application Support/Lodestone/instances/a",
            "instance 7c1f3e8a-0b2d-4f6e-9a11-2d3c4b5a6978 has_token=true",
            "hash 3f786850e387550fdab836ed7e6dc881de23001b",
            "invalid type: map, expected a string at line 1 column 20",
            "the basic settings were saved",
            "unknown variant `Other` in launcher_core::cores::admin::events::token::Kind",
            "kept /Users/steve/mods/SomeVeryLongModNameWithAVersion1-20-1-forge-47.jar",
            "the token expired; signing in again",
            "the tests passed, bypass off, compass ok",
            "max_retries=3 key=5 code: 2",
            "https://example.com:8080/path?page=2 (see https://example.com/a@b)",
            "",
        ] {
            let clean = scrub_line(plain);
            assert!(
                matches!(clean, std::borrow::Cow::Borrowed(_)),
                "{plain} became {clean}"
            );
        }
    }

    #[test]
    fn multi_line_text_is_scrubbed_line_by_line() {
        let text = "first line\npassword: hunter2\n  Authorization: Bearer abc\nlast";
        assert_eq!(
            scrub_line(text),
            "first line\npassword: [redacted]\n  Authorization: [redacted]\nlast"
        );
    }
}
