//! The core's debug traces, read by the owner through `/api/v1/trace/*` (and
//! `lodestone_cli trace`).
//!
//! Local debugging only: events stay in this process (the last
//! [`DEFAULT_BACKLOG`] in memory) and go to the owner's subscriptions. Nothing is
//! written to disk or sent anywhere by this module.
//!
//! What is traced, by topic:
//!
//! - `log.*`: the core's tracing events (`log.error`, `log.warn`, `log.info`, and
//!   `log.debug`/`log.trace` live only), with their fields and spans. Other crates'
//!   records only from info up;
//! - `http.request` (live only) and `http.error`: method, route, status and time of each
//!   API request; never headers, query strings or bodies;
//! - `instance.*`: state changes, warnings and errors, players, system messages; console
//!   output, input and chat live only;
//! - `macro.*`, `task.*` (progress; updates live only), `user.*`, `fs.*`,
//!   `settings.global`: the core's own events.
//!
//! Payloads pass through [`lodestone_trace::scrub`].

use std::{sync::OnceLock, time::Instant};

use axum::{extract::MatchedPath, http::Request, middleware::Next, response::Response};
pub use lodestone_trace::{Event as TraceEvent, Filter, TraceBus, DEFAULT_BACKLOG};
use serde_json::{json, Map, Value};
use tracing::{field::Visit, Level, Subscriber};
use tracing_subscriber::{
    filter::FilterFn,
    layer::{Context, Layer},
    registry::LookupSpan,
};

use crate::events::{Event, EventInner};

/// Busy topics: sent only while the owner listens, and kept out of the backlog.
pub const LIVE_ONLY: &[&str] = &[
    "log.debug",
    "log.trace",
    "http.request",
    "instance.console",
    "instance.input",
    "instance.chat",
    "task.update",
];

/// The core's bus.
pub fn bus() -> &'static TraceBus {
    static BUS: OnceLock<TraceBus> = OnceLock::new();
    BUS.get_or_init(|| TraceBus::new(DEFAULT_BACKLOG, LIVE_ONLY))
}

/// Emits an event; `data` is only made when it goes somewhere.
pub fn emit(topic: &str, data: impl FnOnce() -> Value) {
    bus().emit(topic, data);
}

fn level_topic(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "log.error",
        Level::WARN => "log.warn",
        Level::INFO => "log.info",
        Level::DEBUG => "log.debug",
        Level::TRACE => "log.trace",
    }
}

/// A tracing layer that forwards events to the bus as `log.{level}`.
pub struct TraceLayer;

/// Which records the layer takes: the core's own at any level the bus wants, and other
/// crates' from info up (their debug records describe connections in detail). Checked
/// for each event, so a subscription takes effect at once.
pub fn layer_filter() -> FilterFn<impl Fn(&tracing::Metadata<'_>) -> bool> {
    FilterFn::new(|metadata: &tracing::Metadata<'_>| {
        let own = metadata.target().starts_with("lodestone");
        (own || *metadata.level() <= Level::INFO)
            && (metadata.is_span() || bus().wants(level_topic(metadata.level())))
    })
}

/// Records an event's fields as JSON.
#[derive(Default)]
struct Fields(Map<String, Value>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(
            field.name().to_string(),
            Value::String(format!("{value:?}")),
        );
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
}

impl<S> Layer<S> for TraceLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let topic = level_topic(metadata.level());
        if !bus().wants(topic) {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let message = fields.0.remove("message");
        let spans: Vec<&str> = ctx
            .event_scope(event)
            .map(|scope| scope.from_root().map(|span| span.name()).collect())
            .unwrap_or_default();
        emit(topic, || {
            json!({
                "target": metadata.target(),
                "message": message,
                "fields": fields.0,
                "spans": spans,
                "file": metadata.file(),
                "line": metadata.line(),
            })
        });
    }
}

/// `StateTransition` → `state_transition`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The topic of one of the core's events.
pub fn event_topic(event: &Event) -> String {
    let inner_type = |value: Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            .map(snake)
            .unwrap_or_default()
    };
    match &event.event_inner {
        EventInner::InstanceEvent(e) => {
            let kind = inner_type(json!({ "x": e.instance_event_inner }), "x");
            match kind.as_str() {
                "state_transition" => "instance.state".into(),
                "instance_output" => "instance.console".into(),
                "instance_input" => "instance.input".into(),
                "instance_warning" => "instance.warning".into(),
                "instance_error" => "instance.error".into(),
                "system_message" => "instance.system".into(),
                "player_change" => "instance.players".into(),
                "player_message" => "instance.chat".into(),
                other => format!("instance.{other}"),
            }
        }
        EventInner::UserEvent(e) => {
            format!(
                "user.{}",
                inner_type(json!({ "x": e.user_event_inner }), "x")
            )
        }
        EventInner::MacroEvent(e) => {
            format!(
                "macro.{}",
                inner_type(json!({ "x": e.macro_event_inner }), "x")
            )
        }
        EventInner::ProgressionEvent(e) => {
            match inner_type(json!({ "x": e.progression_event_inner() }), "x").as_str() {
                "progression_start" => "task.start".into(),
                "progression_update" => "task.update".into(),
                "progression_end" => "task.end".into(),
                other => format!("task.{other}"),
            }
        }
        EventInner::FSEvent(_) => "fs.operation".into(),
        EventInner::GlobalSettingsChanged => "settings.global".into(),
    }
}

/// Forwards the core's events (what the dashboard's event stream shows) to the bus.
pub fn forward_events(mut events: tokio::sync::broadcast::Receiver<Event>) {
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    let topic = event_topic(&event);
                    emit(&topic, || {
                        serde_json::to_value(&event).unwrap_or(Value::Null)
                    });
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    emit("trace.lagged", || json!({ "skipped": skipped }));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Middleware: `http.request` (live only) for each API request, `http.error` for
/// answers of 400 and up: method, route (the matched pattern, else the path), status
/// and milliseconds. No query string, header or body.
pub async fn http_middleware(request: Request<axum::body::Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let topic = if status >= 400 {
        "http.error"
    } else {
        "http.request"
    };
    let ms = started.elapsed().as_millis() as u64;
    emit(
        topic,
        || json!({ "method": method, "route": route, "status": status, "ms": ms }),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::{CausedBy, EventInner, InstanceEvent, InstanceEventInner},
        traits::t_server::State,
        types::InstanceUuid,
    };

    fn instance_event(inner: InstanceEventInner) -> Event {
        Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: InstanceUuid::from("a".to_string()),
                instance_name: "Survival".into(),
                instance_event_inner: inner,
            }),
            details: String::new(),
            snowflake: crate::types::Snowflake::default(),
            caused_by: CausedBy::System,
        }
    }

    #[test]
    fn events_get_dotted_topics() {
        assert_eq!(
            event_topic(&instance_event(InstanceEventInner::StateTransition {
                to: State::Running
            })),
            "instance.state"
        );
        assert_eq!(
            event_topic(&instance_event(InstanceEventInner::InstanceOutput {
                message: "Done".into()
            })),
            "instance.console"
        );
        assert_eq!(snake("PlayerChange"), "player_change");
    }
}
