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
    instances::{About, InstancesEvent, LogRow},
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
    /// The modpack import it is about, whose instance didn't exist when it started.
    pub import: Option<u64>,
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
            "import": self.import,
            "data": self.data,
        })
    }

    /// A failure the window showed.
    pub fn is_failure(&self) -> bool {
        matches!(self.kind, "crashed" | "launch_failed" | "server_failed")
    }

    /// Whether it is about the instance `id`. Notices are global, so a job that ends
    /// with one has to pick its own: another instance's verify or crash can come first.
    pub fn is_about(&self, id: &str) -> bool {
        self.instance.as_deref() == Some(id)
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
        self.push_about(kind, title, message, instance, None, data);
    }

    fn push_about(
        &mut self,
        kind: &'static str,
        title: impl Into<String>,
        message: impl Into<String>,
        instance: Option<String>,
        import: Option<u64>,
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
            import,
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
                Some(crash.instance_id.clone()),
                json!({
                    "instance_name": crash.instance_name,
                    "exit_code": crash.exit_code,
                    "report": crash.report,
                    "exception": crash.summary.exception,
                    "description": crash.summary.description,
                    "causes": crash.summary.causes.iter().map(|c| c.explanation()).collect::<Vec<_>>(),
                    "suspects": crash.suspects.iter().map(|s| json!({
                        "file": s.file_name,
                        "name": s.name,
                        "evidence": format!("{:?}", s.evidence),
                    })).collect::<Vec<_>>(),
                    "likely": launcher_core::game::crash::suspects::likely(&crash.suspects),
                    "at_startup": crash.at_startup,
                    "mods_refused": crash.mods_refused,
                }),
            ),
            InstancesEvent::LaunchFailed {
                title,
                message,
                about,
            } => self.push_window_notice("launch_failed", title, message, about),
            InstancesEvent::Notice {
                title,
                message,
                about,
            } => self.push_window_notice("notice", title, message, about),
            InstancesEvent::PlayFailed { id, title, message } => self.push_window_notice(
                "launch_failed",
                title,
                message,
                &About::Instance(id.clone()),
            ),
            // Traced as game.state; whoever cancelled it knows.
            InstancesEvent::PlayCancelled(_) => {}
            // The dialog it opens records itself in traces.
            InstancesEvent::ImportSnapshot(_) => {}
            // Traced as game.window and game.exit.
            InstancesEvent::GameWindowOpened(_) | InstancesEvent::GameExited(_) => {}
            // Traced as game.log; the Friends page shares it when the person chose to.
            InstancesEvent::LanOpened { .. } => {}
            // The dialogs they open record themselves in traces.
            InstancesEvent::OfferMain(_) | InstancesEvent::ConfirmMain { .. } => {}
            // The launch waits for the person's answer in the window: a script
            // following it should know why it doesn't start yet.
            InstancesEvent::ConfirmRosetta {
                id,
                version,
                reason,
            } => self.push(
                "confirm_rosetta",
                format!("Minecraft {version} runs under Rosetta 2"),
                "The game starts once the person presses Play in the launcher's window (asked once per instance), or stops if they cancel.",
                Some(id.clone()),
                json!({ "reason": reason }),
            ),
            InstancesEvent::LoaderChanged(id) => self.push(
                "loader_changed",
                "Loader changed",
                "",
                Some(id.clone()),
                Value::Null,
            ),
            InstancesEvent::Exited { id, stopped } => self.push(
                "exited",
                "Minecraft exited",
                "",
                Some(id.clone()),
                json!({ "stopped": stopped }),
            ),
            InstancesEvent::ReachedMenu(id) => self.push(
                "reached_menu",
                "Minecraft finished loading",
                "",
                Some(id.clone()),
                Value::Null,
            ),
            InstancesEvent::ModsRefused(id) => self.push(
                "mods_refused",
                "The mod loader refused the mods",
                "",
                Some(id.clone()),
                Value::Null,
            ),
        }
    }

    fn push_window_notice(
        &mut self,
        kind: &'static str,
        title: &str,
        message: &str,
        about: &About,
    ) {
        let (instance, import) = match about {
            About::Nothing => (None, None),
            About::Instance(id) => (Some(id.clone()), None),
            About::Import(job) => (None, Some(*job)),
        };
        self.push_about(kind, title, message, instance, import, Value::Null);
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

/// The state names `run_status` gives, for `last_state`.
fn known_state(state: &str) -> Option<&'static str> {
    ["idle", "preparing", "running"]
        .into_iter()
        .find(|known| *known == state)
}

/// Whether a subscription sends an instance's state: when it changed from the one sent
/// (or the one the client said it saw). Instances idle at the start are left out, as
/// nothing happened to them, but not one the client saw doing something.
fn state_event_due(before: Option<&str>, now: &str, first: bool) -> bool {
    before != Some(now) && !(first && before.is_none() && now == "idle")
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
    /// With `instance`: the `cursor` of the `play.logs` the client read, so the rows
    /// logged since then come first, not lost between the two requests.
    #[serde(default)]
    since: Option<u64>,
    /// With `instance`: the state the client last saw it in (`play.logs`' `state`). The
    /// first round sends the state when it differs, idle included, so a game that
    /// exited between the two requests still says so.
    #[serde(default)]
    last_state: Option<String>,
    #[serde(skip)]
    topics_were_empty: bool,
}

pub fn subscribe(call: Call, cx: &mut App) -> Reply {
    let mut params: SubscribeParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    params.topics_were_empty = params.topics.is_empty();
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
    let explicit_log = topics.iter().any(|t| t == "log") && !params.topics_were_empty;
    if explicit_log && !super::game_logs_allowed() {
        return ready(Err(api_error(
            codes::PERMISSION_DENIED,
            "Game logs over the command line are off in Settings → Developer.",
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
    if only.is_none() && (params.since.is_some() || params.last_state.is_some()) {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "since and last_state are about one instance: name it with instance.",
        )));
    }
    let last_state = match params.last_state.as_deref().map(known_state) {
        Some(Some(state)) => Some(state),
        Some(None) => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "last_state is idle, preparing or running.",
            )));
        }
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
            let cursor = match params.since {
                // where the client read up to; never past the tail
                Some(since) if only.as_ref() == Some(&instance.id) => since.min(cursor),
                _ => cursor.saturating_sub(params.backlog as u64),
            };
            cursors.insert(instance.id.clone(), cursor);
        }
    }
    let mut notice_seq = control.notices.borrow().last_seq();
    let mut states: HashMap<String, (&'static str, Value)> = HashMap::new();
    if let (Some(id), Some(state)) = (&only, last_state) {
        states.insert(id.clone(), (state, Value::Null));
    }
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
                        // skipped, not held, while game logs are off
                        if super::game_logs_allowed() {
                            for row in &rows {
                                events.push(("log", row_json(&id, row)));
                            }
                        }
                        cursors.insert(id.clone(), next);
                    }
                    let status = instances.run_status(&id);
                    let progress_now = status
                        .progress
                        .map(|(stage, snapshot)| progress_json(stage.label(), snapshot))
                        .unwrap_or(Value::Null);
                    let before = states.get(&id).cloned();
                    if state && state_event_due(before.as_ref().map(|b| b.0), status.state, first) {
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
            about: About::Instance("a".into()),
        });
        let seq = notices.last_seq();
        notices.instances_event(&InstancesEvent::LaunchFailed {
            title: "Couldn't start Minecraft".into(),
            message: "no Java".into(),
            about: About::Instance("a".into()),
        });
        notices.servers_event(&ServersEvent::Failed {
            title: "Couldn't join".into(),
            message: "gone".into(),
        });
        let since: Vec<&str> = notices.since(seq).iter().map(|n| n.kind).collect();
        assert_eq!(since, ["launch_failed", "server_failed"]);
        // A failed start is a launch failure too, of its instance.
        notices.instances_event(&InstancesEvent::PlayFailed {
            id: "survival".into(),
            title: "Couldn't sign in".into(),
            message: "offline".into(),
        });
        let failed = notices
            .find_since(0, |n| n.title == "Couldn't sign in")
            .unwrap();
        assert!(failed.is_failure());
        assert_eq!(failed.instance.as_deref(), Some("survival"));
        notices.instances_event(&InstancesEvent::PlayCancelled("survival".into()));
        let found = notices
            .find_since(0, |n| n.title.starts_with("Couldn't start"))
            .unwrap();
        assert!(found.is_failure());
        for n in 0..(KEEP + 10) {
            notices.instances_event(&InstancesEvent::Notice {
                title: format!("{n}").into(),
                message: "".into(),
                about: About::Nothing,
            });
        }
        assert_eq!(notices.list.len(), KEEP);
    }

    /// LS-67: a job on one instance ended with another's notice when it came first.
    #[test]
    fn a_job_finds_its_own_notice() {
        let mut notices = Notices::default();
        let seq = notices.last_seq();
        // B's verify ends first, then A's launch fails
        notices.instances_event(&InstancesEvent::Notice {
            title: "Game files verified".into(),
            message: "".into(),
            about: About::Instance("b".into()),
        });
        notices.instances_event(&InstancesEvent::LaunchFailed {
            title: "Couldn't start Minecraft".into(),
            message: "no Java".into(),
            about: About::Instance("a".into()),
        });
        let found = notices
            .find_since(seq, |n| {
                n.is_about("a") && matches!(n.kind, "notice" | "launch_failed" | "crashed")
            })
            .unwrap();
        assert!(found.is_failure());
        assert_eq!(found.instance.as_deref(), Some("a"));

        // a crash says which instance crashed
        let seq = notices.last_seq();
        notices.instances_event(&InstancesEvent::Crashed(crate::instances::CrashInfo {
            instance_id: "b".into(),
            instance_name: "B".into(),
            summary: Default::default(),
            suspects: Vec::new(),
            exit_code: Some(1),
            report: None,
            log_text: String::new(),
            at_startup: false,
            mods_refused: false,
        }));
        notices.instances_event(&InstancesEvent::Notice {
            title: "Game files verified".into(),
            message: "".into(),
            about: About::Instance("a".into()),
        });
        let found = notices
            .find_since(seq, |n| {
                n.is_about("a") && matches!(n.kind, "notice" | "launch_failed" | "crashed")
            })
            .unwrap();
        assert!(!found.is_failure());

        // a modpack import's notice names the import, not an instance
        notices.instances_event(&InstancesEvent::Notice {
            title: "Modpack ready".into(),
            message: "".into(),
            about: About::Import(7),
        });
        let import = notices.find_since(seq, |n| n.import == Some(7)).unwrap();
        assert_eq!(import.title, "Modpack ready");
        assert_eq!(import.instance, None);
    }

    /// LS-69: a game that went idle before a subscription's first round was never
    /// reported, though the client had seen it running.
    #[test]
    fn a_state_the_client_saw_is_compared_on_the_first_round() {
        // idle from the start: nothing to say
        assert!(!state_event_due(None, "idle", true));
        assert!(state_event_due(None, "running", true));
        // the client saw it running, and it exited before the first round
        assert!(state_event_due(Some("running"), "idle", true));
        assert!(!state_event_due(Some("running"), "running", true));
        // later rounds send changes only
        assert!(state_event_due(Some("preparing"), "idle", false));
        assert!(!state_event_due(Some("idle"), "idle", false));
        assert_eq!(known_state("running"), Some("running"));
        assert_eq!(known_state("crashed"), None);
    }
}
