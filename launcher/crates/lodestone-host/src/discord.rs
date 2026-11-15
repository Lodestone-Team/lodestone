//! Discord game invites for shared servers (Settings › Profile › Discord, off unless
//! the owner turns it on): while a server of this computer's core runs and is shared
//! with a Lodestone Connect invite, the owner's Discord status says "Hosting <name>",
//! with the players online and the most there can be, and the invite as its join
//! secret. Discord then shows "Ask to Join" on their profile and lets them send an
//! "Invite to join" card in chat; a friend who presses Join gets the invite in their
//! Lodestone Launcher. See `launcher_core::discord::invite`.
//!
//! When someone asks to join, a toast asks the owner to let them in; unanswered,
//! Discord drops the request after about 30 seconds.
//!
//! Where it costs: nothing while the setting is off (no thread, no requests). While
//! on, the share status of running servers is fetched from the local core when they
//! start, when the Overview changes or reads it, and once every [`SHARE_POLL`] (for
//! shares started or stopped elsewhere). One thread talks to Discord, only while
//! there's a status to show; it sends it only when it changed.

use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, Global, Subscription, Task, WeakEntity};
use launcher_core::{
    cores::admin::{
        instances::InstanceShare,
        models::{Game, InstanceInfo, InstanceState},
    },
    discord::{
        self,
        invite::{CONFIRM_WAIT, HOST_EVENTS, HostCommand, host_loop, join_secret},
        ipc::{Activity, Assets, Connection, JoinRequest, Party, Secrets, Timestamps, field},
    },
};
use lodestone_connect::Invite;
use lodestone_ui::components::Toast;

use crate::{
    i18n::{CoreMsg, t, tr},
    prefs::Prefs,
    state::HostState,
};

/// How often the share status of running servers is fetched again.
pub const SHARE_POLL: Duration = Duration::from_secs(60);
/// How long the "asks to join" toast stays: as long as Discord keeps the request.
const REQUEST_TOAST: Duration = Duration::from_secs(30);
/// The most players a server is shown with when the core doesn't say
/// (Minecraft's default `max-players`).
const DEFAULT_MAX_PLAYERS: u32 = 20;

/// The invites of the shell's state, for [`share_changed`].
struct Current(WeakEntity<DiscordInvites>);

impl Global for Current {}

pub struct DiscordInvites {
    state: WeakEntity<HostState>,
    /// Invite links of shared servers, by instance.
    shares: HashMap<String, String>,
    /// Running servers whose share status was fetched since they started.
    checked: HashSet<String>,
    /// When each server was first shown as hosted (Unix milliseconds).
    since: HashMap<String, u64>,
    /// The server shown, for the "asks to join" toast.
    hosting: Option<String>,
    /// The last status handed to the thread.
    sent: Option<Option<Activity>>,
    worker: Option<mpsc::Sender<HostCommand>>,
    _poll: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl DiscordInvites {
    pub fn new(state: WeakEntity<HostState>, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![cx.observe_global::<Prefs>(|this, cx| this.refresh(cx))];
        if let Some(entity) = state.upgrade() {
            subscriptions.push(cx.observe(&entity, |this, _, cx| this.refresh(cx)));
        }
        let current = Current(cx.entity().downgrade());
        cx.set_global(current);
        let mut this = Self {
            state,
            shares: HashMap::new(),
            checked: HashSet::new(),
            since: HashMap::new(),
            hosting: None,
            sent: None,
            worker: None,
            _poll: None,
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.refresh(cx);
        this
    }

    fn enabled(cx: &App) -> bool {
        cx.try_global::<Prefs>()
            .is_some_and(|prefs| prefs.data.discord_invites)
    }

    /// Fetches what's missing and hands the status to the thread when it changed.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        if !Self::enabled(cx) {
            self.checked.clear();
            self._poll = None;
            self.show(None, cx);
            return;
        }
        let running: Vec<InstanceInfo> = state
            .read(cx)
            .instances
            .iter()
            .flatten()
            .filter(|instance| is_running_java(instance))
            .cloned()
            .collect();
        self.checked
            .retain(|uuid| running.iter().any(|instance| &instance.uuid == uuid));
        let unchecked: Vec<String> = running
            .iter()
            .filter(|instance| !self.checked.contains(&instance.uuid))
            .map(|instance| instance.uuid.clone())
            .collect();
        if !unchecked.is_empty() {
            self.checked.extend(unchecked.iter().cloned());
            self.fetch(unchecked, cx);
        }
        if running.is_empty() {
            self._poll = None;
        } else if self._poll.is_none() {
            self._poll = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(SHARE_POLL).await;
                    let alive = this.update(cx, |this, cx| {
                        let uuids = this.checked.iter().cloned().collect();
                        this.fetch(uuids, cx);
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            }));
        }
        let now = unix_millis();
        let hosted = choose(&running, &self.shares);
        let activity = hosted.and_then(|(instance, link)| {
            let since = *self.since.entry(instance.uuid.clone()).or_insert(now);
            hosting_activity(instance, link, since)
        });
        self.hosting = hosted
            .filter(|_| activity.is_some())
            .map(|(instance, _)| instance.name.clone());
        self.show(activity, cx);
    }

    /// Fetches the share status of `uuids` from the core.
    fn fetch(&mut self, uuids: Vec<String>, cx: &mut Context<Self>) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let admin = state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let results = cx
                .background_spawn(async move {
                    let mut results = Vec::new();
                    for uuid in uuids {
                        let share = admin.share(&uuid).await.ok();
                        results.push((uuid, share));
                    }
                    results
                })
                .await;
            this.update(cx, |this, cx| {
                for (uuid, share) in results {
                    this.set_share(&uuid, share.as_ref());
                }
                this.refresh(cx);
            })
            .ok();
        });
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    fn set_share(&mut self, uuid: &str, share: Option<&InstanceShare>) {
        match share.and_then(shared_link) {
            Some(link) => {
                self.shares.insert(uuid.to_string(), link);
            }
            None => {
                self.shares.remove(uuid);
                self.since.remove(uuid);
            }
        }
    }

    /// Hands `activity` to the thread (starting it the first time there's one), if
    /// it changed.
    fn show(&mut self, activity: Option<Activity>, cx: &mut Context<Self>) {
        if self.sent.as_ref() == Some(&activity) {
            return;
        }
        if self.worker.is_none() {
            if activity.is_none() {
                return;
            }
            self.worker = self.start_worker(cx);
        }
        let Some(worker) = &self.worker else {
            return;
        };
        if worker.send(HostCommand::Show(activity.clone())).is_ok() {
            self.sent = Some(activity);
        }
    }

    fn start_worker(&mut self, cx: &mut Context<Self>) -> Option<mpsc::Sender<HostCommand>> {
        let client_id = discord::client_id()?;
        let (commands, receiver) = mpsc::channel();
        let (requests, mut incoming) = futures::channel::mpsc::unbounded::<JoinRequest>();
        let spawned = std::thread::Builder::new()
            .name("discord-invites".into())
            .spawn(move || {
                host_loop(
                    receiver,
                    std::process::id(),
                    || {
                        Connection::open_subscribed(&client_id, HOST_EVENTS, CONFIRM_WAIT)
                            .map(|(connection, _)| connection)
                    },
                    |request| {
                        let _ = requests.unbounded_send(request);
                    },
                )
            });
        if let Err(err) = spawned {
            eprintln!("lodestone-host: couldn't start the Discord thread: {err}");
            return None;
        }
        self._tasks.push(cx.spawn(async move |this, cx| {
            while let Some(request) = incoming.next().await {
                if this.update(cx, |this, cx| this.ask(request, cx)).is_err() {
                    break;
                }
            }
        }));
        Some(commands)
    }

    /// Someone asks to join on Discord: a toast lets the owner let them in.
    fn ask(&mut self, request: JoinRequest, cx: &mut Context<Self>) {
        let (Some(server), Some(worker), Some(state)) = (
            self.hosting.clone(),
            self.worker.clone(),
            self.state.upgrade(),
        ) else {
            return;
        };
        let text = tr!(
            CoreMsg::DiscordAsksToJoin,
            name = request.name,
            server = server
        );
        let user_id = request.user_id.clone();
        let toast = Toast::info(text)
            .key(format!("discord-join-{}", request.user_id))
            .duration(REQUEST_TOAST)
            .action(t(CoreMsg::DiscordLetJoin), move |_, _| {
                let _ = worker.send(HostCommand::Accept(user_id.clone()));
            });
        state.update(cx, |state, cx| state.toast(toast, cx));
    }
}

/// The Overview read or changed `uuid`'s share status: the status follows at once.
pub fn share_changed(uuid: &str, share: Option<&InstanceShare>, cx: &mut App) {
    let Some(invites) = cx
        .try_global::<Current>()
        .and_then(|current| current.0.upgrade())
    else {
        return;
    };
    if !DiscordInvites::enabled(cx) {
        return;
    }
    invites.update(cx, |invites, cx| {
        invites.set_share(uuid, share);
        invites.refresh(cx);
    });
}

fn is_running_java(instance: &InstanceInfo) -> bool {
    instance.state == InstanceState::Running
        && matches!(instance.game_type, Game::MinecraftJava { .. })
}

/// The invite link of a share that is on.
fn shared_link(share: &InstanceShare) -> Option<String> {
    share
        .link
        .clone()
        .filter(|link| share.sharing && Invite::parse(link).is_ok())
}

/// The server the status shows: the first running one (in the core's order) that is
/// shared.
fn choose<'a>(
    running: &'a [InstanceInfo],
    shares: &'a HashMap<String, String>,
) -> Option<(&'a InstanceInfo, &'a str)> {
    running.iter().find_map(|instance| {
        shares
            .get(&instance.uuid)
            .map(|link| (instance, link.as_str()))
    })
}

/// "Hosting <name>", "Minecraft <version> (2 of 20)", joinable with `link`'s invite.
/// `None` if the invite can't be a join secret.
fn hosting_activity(instance: &InstanceInfo, link: &str, since: u64) -> Option<Activity> {
    let secret = join_secret(&Invite::parse(link).ok()?)?;
    let players = instance.player_count.unwrap_or(0);
    let max = instance.max_player_count.unwrap_or(DEFAULT_MAX_PLAYERS);
    let version = instance.version.trim();
    let state = if version.is_empty() {
        "Minecraft".to_string()
    } else {
        format!("Minecraft {version}")
    };
    let icon = instance
        .icon_url
        .clone()
        .filter(|url| url.starts_with("https://") && url.len() <= 256);
    Some(Activity {
        details: field(tr!(CoreMsg::DiscordHosting, name = instance.name)),
        state: field(state),
        timestamps: Some(Timestamps { start: since }),
        assets: Assets {
            large_image: Some(icon.unwrap_or_else(|| "lodestone".into())),
            large_text: field(&instance.name),
            ..Default::default()
        },
        buttons: Vec::new(),
        party: Party::new(format!("lodestone-{}", instance.uuid), players, max),
        secrets: Secrets::join(secret),
    })
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// Makes the invites for this computer's core's shell (`state`), as its owner.
pub fn create(state: WeakEntity<HostState>, cx: &mut App) -> Entity<DiscordInvites> {
    cx.new(|cx| DiscordInvites::new(state, cx))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn instance(uuid: &str, state: &str, players: Option<u32>) -> InstanceInfo {
        serde_json::from_value(json!({
            "uuid": uuid,
            "name": "Survival",
            "game_type": { "type": "MinecraftJava", "variant": { "type": "Vanilla" } },
            "version": "1.21.4",
            "port": 25565,
            "state": state,
            "player_count": players,
            "max_player_count": 10,
        }))
        .expect("an instance")
    }

    fn link() -> String {
        format!(
            "https://connect.lodestone.cc/j/7KQ2M9XB4D#{}.{}",
            "3f".repeat(32),
            "c0".repeat(16)
        )
    }

    #[test]
    fn a_shared_running_server_is_joinable() {
        let server = instance("a", "Running", Some(3));
        assert!(is_running_java(&server));
        let activity = hosting_activity(&server, &link(), 1000).expect("a status");
        let value = serde_json::to_value(&activity).unwrap();
        assert_eq!(value["details"], "Hosting Survival");
        assert_eq!(value["state"], "Minecraft 1.21.4");
        assert_eq!(
            value["party"],
            json!({ "id": "lodestone-a", "size": [3, 10] })
        );
        assert_eq!(value["assets"]["large_image"], "lodestone");
        assert_eq!(value["timestamps"]["start"], 1000);
        // the invite, written to fit, and nothing else in the secrets
        let secret = value["secrets"]["join"].as_str().unwrap();
        assert!(secret.len() <= 128);
        let invite = launcher_core::discord::invite::invite_from_secret(secret).unwrap();
        assert_eq!(invite.link(), link());
        assert_eq!(value["secrets"].as_object().unwrap().len(), 1);
        assert!(value.get("buttons").is_none());
        // a server without a player count yet shows 0
        let starting = instance("a", "Running", None);
        let activity = hosting_activity(&starting, &link(), 0).unwrap();
        assert_eq!(activity.party.unwrap().size, [0, 10]);
        // not an invite: nothing
        assert!(hosting_activity(&server, "https://example.com", 0).is_none());
    }

    #[test]
    fn the_first_shared_running_server_is_shown() {
        let running = vec![
            instance("a", "Running", Some(1)),
            instance("b", "Running", Some(2)),
        ];
        let mut shares = HashMap::new();
        assert!(choose(&running, &shares).is_none());
        shares.insert("b".to_string(), link());
        assert_eq!(choose(&running, &shares).unwrap().0.uuid, "b");
        shares.insert("a".to_string(), link());
        assert_eq!(choose(&running, &shares).unwrap().0.uuid, "a");
        assert!(!is_running_java(&instance("c", "Stopped", None)));
        assert!(!is_running_java(&instance("c", "Starting", None)));
    }

    #[test]
    fn only_a_share_that_is_on_has_a_link() {
        let mut share = InstanceShare {
            sharing: true,
            link: Some(link()),
            ..Default::default()
        };
        assert_eq!(shared_link(&share), Some(link()));
        share.sharing = false;
        assert_eq!(shared_link(&share), None);
        share.sharing = true;
        share.link = None;
        assert_eq!(shared_link(&share), None);
    }
}
