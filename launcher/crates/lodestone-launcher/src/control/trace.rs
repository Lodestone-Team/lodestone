//! `trace.*`: the launcher's debug traces (see [`launcher_core::trace`]) for
//! `lodestone-launcher-cli trace`. The socket answers this user only, and the events
//! never leave the machine.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::App;
use launcher_control::{Message, server::Outbox};
use launcher_core::trace::{self, Event, Filter};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, Reply, api_error, instances::summaries, ready};

/// The topics the launcher emits, with what each says. `trace.topics` adds how many
/// events each had so far.
pub const CATALOG: &[(&str, &str)] = &[
    (
        "ui.click",
        "A shared control was clicked: button, icon button, list row (id, label).",
    ),
    ("ui.toggle", "A switch was flipped (id, value)."),
    (
        "ui.select",
        "A tab, pill or segmented choice was picked (id, index, label).",
    ),
    (
        "ui.action",
        "A key ran an action (action name, keys). Typed text is never traced.",
    ),
    ("ui.key", "A shortcut that ran nothing (live only)."),
    (
        "ui.pointer",
        "Where the pointer went down (x, y, button, clicks).",
    ),
    ("ui.modal", "A dialog opened or closed (modal)."),
    (
        "ui.scope",
        "A popover, menu or dialog took focus (open) or was dismissed.",
    ),
    (
        "ui.settings",
        "The Settings dialog opened, closed or changed page.",
    ),
    ("ui.window", "The window was resized."),
    ("ui.zoom", "The interface zoom changed."),
    ("nav.page", "The main page changed (instance, servers)."),
    (
        "nav.tab",
        "The instance tab changed (play, content, worlds, settings, logs).",
    ),
    ("nav.instance", "Another instance was selected."),
    ("nav.account", "Another account is in use."),
    (
        "settings.change",
        "A saved setting changed (scope launcher/instance/shared, path, from, to).",
    ),
    (
        "game.prepare",
        "Play was pressed: installing what the game needs.",
    ),
    ("game.launch", "The game process started (pid)."),
    ("game.log", "A console line (live only)."),
    (
        "game.exit",
        "The game exited (code, stopped, crashed, secs).",
    ),
    ("game.failed", "The game couldn't start (title, message)."),
    ("game.state", "The instance went idle."),
    ("job.started", "A control API job started."),
    ("job.finished", "A control API job ended (state, error)."),
    (
        "http.response",
        "An HTTP request finished (method, host, path, status, ms).",
    ),
    ("http.error", "An HTTP request failed."),
    (
        "storage.clean",
        "Settings > Storage removed files (what, freed_bytes, files, error).",
    ),
    ("download.file", "A game file was downloaded (live only)."),
    ("download.error", "A game file download failed."),
    (
        "core_http.response",
        "A request to a Lodestone core finished.",
    ),
    ("core_http.error", "A request to a Lodestone core failed."),
    ("log.error", "An error the launcher or GPUI logged."),
    ("log.warn", "A warning."),
    ("log.info", "An info record."),
    ("log.debug", "A debug record (live only)."),
    ("log.trace", "A trace record (live only)."),
    (
        "perf.frame",
        "The window redrew (gap since the last frame, live only).",
    ),
];

pub fn topics(_: Call, _: &mut App) -> Reply {
    let seen = trace::bus().topics();
    let count = |topic: &str| seen.iter().find(|(t, _)| t == topic).map_or(0, |(_, n)| *n);
    let mut list: Vec<Value> = CATALOG
        .iter()
        .map(|(topic, about)| {
            json!({
                "topic": topic,
                "about": about,
                "live_only": trace::LIVE_ONLY.iter().any(|p| topic.starts_with(p)),
                "events": count(topic),
            })
        })
        .collect();
    // anything emitted that the catalog doesn't name yet
    for (topic, events) in &seen {
        if !CATALOG.iter().any(|(t, _)| t == topic) {
            list.push(json!({ "topic": topic, "about": "", "live_only": false, "events": events }));
        }
    }
    ready(Ok(json!({
        "topics": list,
        "backlog": trace::DEFAULT_BACKLOG,
        "last_seq": trace::bus().last_seq(),
        "patterns": "ui or ui.* takes everything under ui; * is one segment (*.error); * alone takes everything",
    })))
}

#[derive(Deserialize)]
struct SubscribeParams {
    #[serde(default)]
    topics: Vec<String>,
    /// How many earlier events (that match) to send first.
    #[serde(default)]
    backlog: Option<usize>,
    /// Only earlier events after this sequence number.
    #[serde(default)]
    since: Option<u64>,
}

/// Events held until the subscription's response has gone out: the client only
/// reads events after it.
struct Gate {
    open: bool,
    held: Vec<Arc<Event>>,
}

fn send(outbox: &Outbox, number: u64, event: &Event) -> bool {
    outbox.send(&Message::Event {
        subscription: number,
        event: "trace".into(),
        data: event.to_json(),
    })
}

pub fn subscribe(call: Call, cx: &mut App) -> Reply {
    let params: SubscribeParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let control = call.control.clone();
    let number = control.reserve_subscription();
    let outbox = call.outbox.clone();
    let gate = Arc::new(Mutex::new(Gate {
        open: false,
        held: Vec::new(),
    }));
    let sink = {
        let (gate, outbox) = (gate.clone(), outbox.clone());
        Box::new(move |event: &Event| {
            let mut gate = gate.lock().unwrap_or_else(|p| p.into_inner());
            if gate.open {
                send(&outbox, number, event)
            } else {
                gate.held.push(Arc::new(event.clone()));
                !outbox.is_closed()
            }
        })
    };
    let filter = Filter::new(params.topics.iter());
    let backlog = params.backlog.unwrap_or(0);
    let id = trace::bus().subscribe(filter, backlog, params.since, sink);
    control.insert_trace_subscription(call.connection, number, id);
    // once the response is out (it is queued when this handler returns), the held
    // events follow in order
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        let mut gate = gate.lock().unwrap_or_else(|p| p.into_inner());
        for event in std::mem::take(&mut gate.held) {
            send(&outbox, number, &event);
        }
        gate.open = true;
    })
    .detach();
    ready(Ok(json!({
        "subscription": number,
        "topics": params.topics,
        "last_seq": trace::bus().last_seq(),
    })))
}

#[derive(Deserialize)]
struct UnsubscribeParams {
    subscription: u64,
}

pub fn unsubscribe(call: Call, _: &mut App) -> Reply {
    let params: UnsubscribeParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    if call
        .control
        .remove_trace_subscription(call.connection, params.subscription)
    {
        ready(Ok(
            json!({ "subscription": params.subscription, "ended": true }),
        ))
    } else {
        ready(Err(api_error(
            launcher_control::codes::NOT_FOUND,
            format!(
                "No trace subscription {} on this connection.",
                params.subscription
            ),
        )))
    }
}

/// What the launcher shows and does now: the window's state as traced (page, tab,
/// selected instance, dialog, Settings page, window size, zoom, account), each
/// instance's game state, and the running jobs.
pub fn snapshot(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let jobs: Vec<Value> = {
        let jobs = control.jobs.borrow();
        let instances = control.instances.read(cx);
        jobs.states()
            .into_iter()
            .filter(|(_, state)| *state == super::JobState::Running)
            .filter_map(|(id, _)| jobs.get_json(id, instances))
            .collect()
    };
    ready(Ok(json!({
        "window": trace::bus().state(),
        "instances": summaries(control, cx),
        "running_jobs": jobs,
        "trace": {
            "last_seq": trace::bus().last_seq(),
            "subscribers": trace::bus().subscribers(),
        },
        "pid": std::process::id(),
        "uptime_secs": control.started.elapsed().as_secs(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_names_topics_once() {
        let mut names: Vec<&str> = CATALOG.iter().map(|(t, _)| *t).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
        for live in trace::LIVE_ONLY {
            assert!(
                CATALOG.iter().any(|(t, _)| t.starts_with(live)),
                "{live} has no entry"
            );
        }
    }
}
