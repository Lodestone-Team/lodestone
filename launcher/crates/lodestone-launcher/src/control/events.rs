//! What happens without being asked: the notices the window shows (a crash, a failed
//! launch, a finished export), and subscriptions that stream log lines, game states,
//! progress, jobs and notices.

use std::{collections::HashMap, collections::VecDeque, time::Duration};

use gpui::App;
use launcher_control::{Message, codes};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, JobState, Reply, api_error, jobs::progress_json, ready};
use crate::{
    instances::{InstancesEvent, LogRow},
    servers::ServersEvent,
};

/// Notices kept for subscriptions and for jobs that end with one.
const KEEP: usize = 200;

/// Something the window told the user.
#[derive(Clone, Debug)]
pub struct Notice {
    pub seq: u64,
    pub at: u64,
    /// `crashed`, `launch_failed`, `notice`, `loader_changed`, `server_failed`,
    /// `open_instance`.
    pub kind: &'static str,
    pub title: String,
    pub message: String,
    pub instance: Option<String>,
    pub data: Value,
}

impl Notice {
    pub fn to_json(&self) -> Value {
        json!({
            "seq": self.seq,
            "at": self.at,
            "kind": self.kind,
            "title": self.title,
            "message": self.message,
            "instance": self.instance,
            "data": self.data,
        })
    }

    /// A failure the window showed.
    pub fn is_failure(&self) -> bool {
        matches!(self.kind, "crashed" | "launch_failed" | "server_failed")
    }
}

#[derive(Default)]
pub struct Notices {
    next: u64,
    list: VecDeque<Notice>,
}

impl Notices {
    fn push(
        &mut self,
        kind: &'static str,
        title: impl Into<String>,
        message: impl Into<String>,
        instance: Option<String>,
        data: Value,
    ) {
        self.next += 1;
        self.list.push_back(Notice {
            seq: self.next,
            at: launcher_core::instances::now(),
            kind,
            title: title.into(),
            message: message.into(),
            instance,
            data,
        });
        while self.list.len() > KEEP {
            self.list.pop_front();
        }
    }

    pub fn instances_event(&mut self, event: &InstancesEvent) {
        match event {
            InstancesEvent::Crashed(crash) => self.push(
                "crashed",
                format!("{} crashed", crash.instance_name),
                crash
                    .summary
                    .exception
                    .clone()
                    .or_else(|| crash.summary.description.clone())
                    .unwrap_or_else(|| "Minecraft crashed.".into()),
                None,
                json!({
                    "instance_name": crash.instance_name,
                    "exit_code": crash.exit_code,
                    "report": crash.report,
                    "exception": crash.summary.exception,
                    "description": crash.summary.description,
                    "causes": crash.summary.causes.iter().map(|c| c.explanation()).collect::<Vec<_>>(),
                }),
            ),
            InstancesEvent::LaunchFailed { title, message } => {
                self.push("launch_failed", title.as_ref(), message.as_ref(), None, Value::Null)
            }
            InstancesEvent::Notice { title, message } => {
                self.push("notice", title.as_ref(), message.as_ref(), None, Value::Null)
            }
            InstancesEvent::LoaderChanged(id) => self.push(
                "loader_changed",
                "Loader changed",
                "",
                Some(id.clone()),
                Value::Null,
            ),
        }
    }

    pub fn servers_event(&mut self, event: &ServersEvent) {
        match event {
            ServersEvent::OpenInstance(id) => self.push(
                "open_instance",
                "Joining",
                "",
                Some(id.clone()),
                Value::Null,
            ),
            ServersEvent::Failed { title, message } => self.push(
                "server_failed",
                title.as_ref(),
                message.as_ref(),
                None,
                Value::Null,
            ),
            ServersEvent::ShowServers => {}
        }
    }

    pub fn last_seq(&self) -> u64 {
        self.next
    }

    pub fn since(&self, seq: u64) -> Vec<Notice> {
        self.list.iter().filter(|n| n.seq > seq).cloned().collect()
    }

    /// The first notice after `seq` that `matches`.
    pub fn find_since(&self, seq: u64, matches: impl Fn(&Notice) -> bool) -> Option<Notice> {
        self.list
            .iter()
            .find(|n| n.seq > seq && matches(n))
            .cloned()
    }
}

/// A console row as the API sends it.
pub fn row_json(instance: &str, row: &LogRow) -> Value {
    json!({
        "instance": instance,
        "level": row.level.to_string(),
        "time": row.time.as_ref().map(|t| t.to_string()),
        "thread": row.source.as_ref().map(|t| t.to_string()),
        "stream": format!("{:?}", row.stream).to_lowercase(),
        "text": row.text.to_string(),
    })
}

/// Every topic.
pub const TOPICS: [&str; 5] = ["log", "state", "progress", "jobs", "notices"];

#[derive(Deserialize)]
struct SubscribeParams {
    #[serde(default)]
    topics: Vec<String>,
    /// One instance's log, state and progress (id or name); all instances without it.
    #[serde(default)]
    instance: Option<String>,
    /// Console rows from before the subscription to send first.
    #[serde(default)]
    backlog: usize,
}

pub fn subscribe(call: Call, cx: &mut App) -> Reply {
    let params: SubscribeParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let topics: Vec<String> = if params.topics.is_empty() {
        TOPICS.iter().map(|t| t.to_string()).collect()
    } else {
        params.topics
    };
    if let Some(bad) = topics.iter().find(|t| !TOPICS.contains(&t.as_str())) {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            format!("No topic \"{bad}\". Topics: {}.", TOPICS.join(", ")),
        )));
    }
    let control = call.control.clone();
    let only = match &params.instance {
        Some(reference) => match control.instance(reference, cx) {
            Ok(instance) => Some(instance.id),
            Err(err) => return ready(Err(err)),
        },
        None => None,
    };
    let wants = |topic: &str| topics.iter().any(|t| t == topic);
    let (log, state, progress, jobs, notices) = (
        wants("log"),
        wants("state"),
        wants("progress"),
        wants("jobs"),
        wants("notices"),
    );
    // where each instance's log is now, less the backlog
    let mut cursors: HashMap<String, u64> = HashMap::new();
    {
        let instances = control.instances.read(cx);
        for instance in instances.list() {
            let (_, cursor) = instances.log_tail(&instance.id, 0);
            cursors.insert(
                instance.id.clone(),
                cursor.saturating_sub(params.backlog as u64),
            );
        }
    }
    let mut notice_seq = control.notices.borrow().last_seq();
    let mut states: HashMap<String, (&'static str, Value)> = HashMap::new();
    let mut job_states: HashMap<u64, (JobState, Value)> = HashMap::new();
    let number = control.reserve_subscription();
    let outbox = call.outbox.clone();
    let task_control = control.clone();
    let task = cx.spawn(async move |cx| {
        let control = task_control;
        let mut first = true;
        loop {
            // the first round waits too, so the response goes out before any event
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let events: Vec<(&'static str, Value)> = cx.update(|cx| {
                let mut events = Vec::new();
                let instances = control.instances.read(cx);
                for instance in instances.list() {
                    if only.as_ref().is_some_and(|id| id != &instance.id) {
                        continue;
                    }
                    let id = instance.id.clone();
                    if log {
                        let cursor = cursors.get(&id).copied().unwrap_or(0);
                        let (rows, next) = instances.log_since(&id, cursor);
                        for row in &rows {
                            events.push(("log", row_json(&id, row)));
                        }
                        cursors.insert(id.clone(), next);
                    }
                    let status = instances.run_status(&id);
                    let progress_now = status
                        .progress
                        .map(|(stage, snapshot)| progress_json(stage.label(), snapshot))
                        .unwrap_or(Value::Null);
                    let before = states.get(&id).cloned();
                    if state
                        && before.as_ref().map(|b| b.0) != Some(status.state)
                        && !(first && status.state == "idle")
                    {
                        events.push((
                            "state",
                            json!({
                                "instance": id,
                                "name": instance.name,
                                "state": status.state,
                                "previous": before.as_ref().map(|b| b.0),
                                "pid": status.pid,
                            }),
                        ));
                    }
                    if progress
                        && !progress_now.is_null()
                        && before.as_ref().map(|b| &b.1) != Some(&progress_now)
                    {
                        let mut data = progress_now.clone();
                        data["instance"] = json!(id);
                        events.push(("progress", data));
                    }
                    states.insert(id, (status.state, progress_now));
                }
                if jobs || progress {
                    let all = control.jobs.borrow();
                    for (id, job_state) in all.states() {
                        let Some(json) = all.get_json(id, instances) else {
                            continue;
                        };
                        let before = job_states.get(&id).cloned();
                        let changed_state = before.as_ref().map(|b| b.0) != Some(job_state);
                        let changed_progress =
                            before.as_ref().map(|b| &b.1) != Some(&json["progress"]);
                        let fresh = before.is_none() && first;
                        if jobs && changed_state && !fresh {
                            events.push(("job", json.clone()));
                        } else if progress
                            && changed_progress
                            && job_state == JobState::Running
                            && !fresh
                        {
                            events.push(("job_progress", json.clone()));
                        }
                        job_states.insert(id, (job_state, json["progress"].clone()));
                    }
                }
                if notices {
                    for notice in control.notices.borrow().since(notice_seq) {
                        notice_seq = notice.seq;
                        events.push(("notice", notice.to_json()));
                    }
                }
                events
            });
            first = false;
            for (event, data) in events {
                let sent = outbox.send(&Message::Event {
                    subscription: number,
                    event: event.to_string(),
                    data,
                });
                if !sent {
                    return;
                }
            }
            let dropped = outbox.take_dropped();
            if dropped > 0 {
                outbox.send(&Message::Event {
                    subscription: number,
                    event: "lagged".into(),
                    data: json!({ "dropped": dropped }),
                });
            }
            if outbox.is_closed() {
                return;
            }
        }
    });
    control.insert_subscription(call.connection, number, task);
    ready(Ok(json!({ "subscription": number, "topics": topics })))
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
        .remove_subscription(call.connection, params.subscription)
    {
        ready(Ok(
            json!({ "subscription": params.subscription, "ended": true }),
        ))
    } else {
        ready(Err(api_error(
            codes::NOT_FOUND,
            format!(
                "No subscription {} on this connection.",
                params.subscription
            ),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_are_kept_in_order() {
        let mut notices = Notices::default();
        notices.instances_event(&InstancesEvent::Notice {
            title: "Modpack exported".into(),
            message: "3 file(s)".into(),
        });
        let seq = notices.last_seq();
        notices.instances_event(&InstancesEvent::LaunchFailed {
            title: "Couldn't start Minecraft".into(),
            message: "no Java".into(),
        });
        notices.servers_event(&ServersEvent::Failed {
            title: "Couldn't join".into(),
            message: "gone".into(),
        });
        let since: Vec<&str> = notices.since(seq).iter().map(|n| n.kind).collect();
        assert_eq!(since, ["launch_failed", "server_failed"]);
        let found = notices
            .find_since(0, |n| n.title.starts_with("Couldn't start"))
            .unwrap();
        assert!(found.is_failure());
        for n in 0..(KEEP + 10) {
            notices.instances_event(&InstancesEvent::Notice {
                title: format!("{n}").into(),
                message: "".into(),
            });
        }
        assert_eq!(notices.list.len(), KEEP);
    }
}
