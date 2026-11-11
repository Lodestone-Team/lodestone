//! "Share LAN worlds with friends" (launcher/docs/friends.md, "LAN worlds"): while the
//! setting is on, the worlds this computer opens to LAN show on friends' Friends pages,
//! and friends can join them through Lodestone Connect.
//!
//! - **Detecting** ([`launcher_core::cores::lan_open`]): LAN announcements from this
//!   computer (heard on the multicast group, whoever launched the game), and
//!   `Started serving on <port>` in the logs of games this launcher started
//!   ([`InstancesEvent::LanOpened`]). A world counts once 127.0.0.1:<port> answers a
//!   status ping; it closes when its announcements stop, its game exits, or its port
//!   stops answering.
//! - **Publishing**: turning the setting on lists this launcher in the person's state
//!   as a launcher sharing LAN worlds ([`Friends::start_lan_sharing`]), once; turning it
//!   off takes it out. Worlds opening and closing need no new state: the friend host
//!   runs only while a world is open, so friends' presence checks say "open now".
//! - **Serving** ([`LanHost`]): accepted friends the person doesn't hide from, with the
//!   core secrets of the launcher's certificate. Hiding a friend or removing them
//!   updates it at once (the friend list's copy changes, and the next tick sees it).
//!
//! It runs whichever page is shown (the person is in the game, not on the Friends page),
//! never in the offscreen screenshots, and only while the setting is on. With the
//! setting off nothing listens, publishes or serves.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::{
    FutureExt as _, StreamExt as _,
    channel::mpsc,
    future::{self, Either},
};
use gpui::{AppContext as _, Context, Task};
use launcher_core::{
    accounts::AccountKind,
    cores::{
        lan::{self, LanListener},
        lan_open::{self, GameInfo, OpenWorld, OpenWorlds},
    },
    friend_list::{
        self,
        lan::{
            FriendKey, LanFriend as FriendAccess, LanHost, LanHostConfig, LanHostState,
            LanIdentity as CoreIdentity, LanKnocks as KnockMemory, lan_friends, lan_launcher_of,
        },
    },
};

use super::{Servers, friend_list::hex};
use crate::instances::InstancesEvent;

/// How often the open worlds are looked at while there are any (or a host runs).
const TICK: Duration = Duration::from_secs(2);
/// How long after a failed registration (no friend list yet, offline) it's tried again.
const REGISTER_AGAIN_AFTER: Duration = Duration::from_secs(60);
/// How often it looks with no world open: for a registration to try again, or a
/// certificate that left the state. A world opening wakes it at once, by its
/// announcements or its game's log.
const IDLE_TICK: Duration = REGISTER_AGAIN_AFTER;

/// What the Friends page shows of the person's own sharing.
#[derive(Default)]
pub struct LanShare {
    /// The setting, as last applied.
    pub enabled: bool,
    listener: Option<LanListener>,
    worlds: Arc<Mutex<OpenWorlds>>,
    /// The open worlds, as last worked out (shown under "You").
    pub open: Vec<OpenWorld>,
    pinging: bool,
    /// This launcher's certificate, once listed in the person's state.
    identity: Option<CoreIdentity>,
    registering: bool,
    registered_at: Option<Instant>,
    host: Option<LanHost>,
    /// The friends the host was last given: (key, secret's first bytes) to notice
    /// changes without keeping secrets around twice.
    host_friends: Vec<String>,
    /// How the host is doing, while it runs.
    pub state: Option<LanHostState>,
    /// Why sharing can't go on: no friend list yet, or the service refused.
    pub problem: Option<String>,
    knocks: KnockMemory,
    ticker: Option<Task<()>>,
    /// Wakes the ticker out of its long wait with nothing open.
    wake: Option<mpsc::UnboundedSender<()>>,
    /// The offscreen screenshots fill these themselves.
    pub preview: bool,
}

impl Servers {
    /// Applies the setting: starts or stops sharing when it changed.
    pub(super) fn lan_share_apply_setting(&mut self, cx: &mut Context<Self>) {
        if self.preview || self.lan_share.preview {
            return;
        }
        let on = self.instances.read(cx).settings().share_lan_worlds;
        if on == self.lan_share.enabled {
            return;
        }
        if on {
            self.lan_share_start(cx);
        } else {
            self.lan_share_stop(cx);
        }
    }

    /// The switch on the Friends page.
    pub fn set_share_lan_worlds(&mut self, on: bool, cx: &mut Context<Self>) {
        self.instances
            .update(cx, |instances, cx| instances.set_share_lan_worlds(on, cx));
        self.lan_share_apply_setting(cx);
    }

    fn lan_share_start(&mut self, cx: &mut Context<Self>) {
        let share = &mut self.lan_share;
        share.enabled = true;
        share.problem = None;
        share.identity = None;
        share.registered_at = None;
        // announcements from this computer only: asked once per address
        let worlds = share.worlds.clone();
        let local: Mutex<HashMap<IpAddr, bool>> = Mutex::default();
        // announcements from this computer, and games this launcher started, wake the
        // ticker, which waits long while nothing is open
        let (wake, woken) = mpsc::unbounded::<()>();
        share.wake = Some(wake.clone());
        let listening = lan::lan_socket().and_then(|socket| {
            lan::listen(socket, move |host, announcement| {
                let mine = local
                    .lock()
                    .map(|mut known| {
                        *known
                            .entry(host)
                            .or_insert_with(|| lan_open::is_this_computer(host))
                    })
                    .unwrap_or(false);
                if mine && let Ok(mut worlds) = worlds.lock() {
                    worlds.announced(announcement.port, announcement.motd, Instant::now());
                    let _ = wake.unbounded_send(());
                }
                true
            })
        });
        // without announcements (no Local Network permission, say), worlds of games
        // this launcher started still show, from their logs
        share.listener = listening.ok();
        share.ticker = Some(cx.spawn(async move |this, cx| {
            // `None` once every sender is gone: timers only
            let mut woken = Some(woken);
            loop {
                let Ok(busy) = this.update(cx, |this, cx| {
                    this.lan_share_tick(cx);
                    this.lan_share_busy()
                }) else {
                    break;
                };
                let timer = cx
                    .background_executor()
                    .timer(if busy { TICK } else { IDLE_TICK });
                let closed = match woken.as_mut() {
                    Some(rx) => {
                        let closed = matches!(
                            future::select(timer, rx.next()).await,
                            Either::Right((None, _))
                        );
                        // one tick for any number of announcements
                        while rx.try_recv().is_ok() {}
                        closed
                    }
                    None => {
                        timer.await;
                        false
                    }
                };
                if closed {
                    woken = None;
                }
            }
        }));
        // the friend list, for the friends to share with (opens it, making nothing)
        self.friend_peek(cx).detach();
        self.lan_share_register(cx);
        cx.notify();
    }

    fn lan_share_stop(&mut self, cx: &mut Context<Self>) {
        let share = &mut self.lan_share;
        share.enabled = false;
        share.listener = None;
        share.ticker = None;
        share.wake = None;
        share.host = None;
        share.state = None;
        share.host_friends.clear();
        share.identity = None;
        share.open.clear();
        share.problem = None;
        if let Ok(mut worlds) = share.worlds.lock() {
            *worlds = OpenWorlds::default();
        }
        // friends' launchers stop listing this one with the next state
        let task = self.friend_op(
            cx,
            false,
            Box::new(|friends| {
                async move {
                    if friends.stop_lan_sharing()? {
                        let _ = friends.publish_states().await;
                    }
                    Ok(())
                }
                .boxed()
            }),
        );
        task.detach();
        cx.notify();
    }

    /// Lists this launcher in the person's state (once per start, and again when its
    /// certificate was retired from the list).
    fn lan_share_register(&mut self, cx: &mut Context<Self>) {
        if self.lan_share.registering {
            return;
        }
        self.lan_share.registering = true;
        self.lan_share.registered_at = Some(Instant::now());
        let task = self.friend_op(
            cx,
            false,
            Box::new(|friends| {
                async move {
                    let (identity, changed) = friends.start_lan_sharing()?;
                    if changed {
                        // friends who can't be reached get it with the next refresh
                        let _ = friends.publish_states().await;
                    }
                    Ok(identity)
                }
                .boxed()
            }),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let share = &mut this.lan_share;
                share.registering = false;
                if !share.enabled {
                    return;
                }
                match result {
                    Ok(identity) => {
                        share.problem = None;
                        if share
                            .identity
                            .as_ref()
                            .is_some_and(|old| old.fingerprint() != identity.fingerprint())
                        {
                            // a new certificate: friends pin the new one
                            share.host = None;
                        }
                        share.identity = Some(identity);
                    }
                    Err(err) => share.problem = Some(err),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A game this launcher started opened a world to LAN, or exited.
    pub(super) fn lan_share_event(&mut self, event: &InstancesEvent, cx: &mut Context<Self>) {
        if !self.lan_share.enabled {
            return;
        }
        match event {
            InstancesEvent::LanOpened { id, port } => {
                let info = {
                    let instances = self.instances.read(cx);
                    let Some(instance) = instances.list().iter().find(|i| &i.id == id) else {
                        return;
                    };
                    // the account it runs with: the active one, as at launch
                    let online = self
                        .accounts
                        .read(cx)
                        .active()
                        .map(|account| account.kind != AccountKind::Offline);
                    GameInfo {
                        instance: id.clone(),
                        game_version: instance.game_version.clone(),
                        loader: loader_name(instance.loader).to_string(),
                        loader_version: instance.loader_version.clone(),
                        online_mode: online,
                    }
                };
                if let Ok(mut worlds) = self.lan_share.worlds.lock() {
                    worlds.game_serving(*port, info);
                }
                self.lan_share_tick(cx);
                self.lan_share_wake();
            }
            InstancesEvent::GameExited(id) | InstancesEvent::Exited { id, .. } => {
                if let Ok(mut worlds) = self.lan_share.worlds.lock() {
                    worlds.game_exited(id);
                }
                self.lan_share_tick(cx);
                self.lan_share_wake();
            }
            InstancesEvent::Crashed(crash) => {
                if let Ok(mut worlds) = self.lan_share.worlds.lock() {
                    worlds.game_exited(&crash.instance_id);
                }
                self.lan_share_tick(cx);
                self.lan_share_wake();
            }
            _ => {}
        }
    }

    /// Has the ticker look again now, and at its short interval while worlds are open.
    fn lan_share_wake(&self) {
        if let Some(wake) = &self.lan_share.wake {
            let _ = wake.unbounded_send(());
        }
    }

    /// Whether the ticker should look again soon: worlds named or being pinged, or a
    /// host running.
    fn lan_share_busy(&self) -> bool {
        let share = &self.lan_share;
        share.pinging
            || share.host.is_some()
            || share
                .worlds
                .lock()
                .map_or(true, |worlds| !worlds.is_empty())
    }

    /// Pings the ports that are due, works out the open worlds, and starts, updates or
    /// stops the host to match.
    fn lan_share_tick(&mut self, cx: &mut Context<Self>) {
        if !self.lan_share.enabled {
            return;
        }
        let now = Instant::now();
        let due = match self.lan_share.worlds.lock() {
            Ok(mut worlds) => {
                worlds.expire(now);
                worlds.due(now)
            }
            Err(_) => Vec::new(),
        };
        if !due.is_empty() && !self.lan_share.pinging {
            self.lan_share.pinging = true;
            let pings = cx.background_spawn(async move {
                due.into_iter()
                    .map(|port| (port, lan_open::ping(port, lan_open::PING_TIMEOUT).ok()))
                    .collect::<Vec<_>>()
            });
            cx.spawn(async move |this, cx| {
                let results = pings.await;
                this.update(cx, |this, cx| {
                    this.lan_share.pinging = false;
                    if let Ok(mut worlds) = this.lan_share.worlds.lock() {
                        for (port, status) in results {
                            worlds.checked(port, status, Instant::now());
                        }
                    }
                    this.lan_share_reconcile(cx);
                })
                .ok();
            })
            .detach();
        }
        // the certificate left the state (retired from "You", or another app's copy
        // of the list dropped it): list it again, with a new one if it was retired
        let snapshot = &self.friend_list.snapshot;
        if self.friend_list.opened
            && !self.lan_share.registering
            && let Some(identity) = &self.lan_share.identity
            && lan_launcher_of(&snapshot.book)
                .is_none_or(|core| core.fingerprint != hex(&identity.fingerprint()))
        {
            self.lan_share.identity = None;
            self.lan_share_register(cx);
        }
        if self.lan_share.identity.is_none()
            && !self.lan_share.registering
            && self
                .lan_share
                .registered_at
                .is_none_or(|at| at.elapsed() >= REGISTER_AGAIN_AFTER)
        {
            self.lan_share_register(cx);
        }
        self.lan_share_reconcile(cx);
    }

    /// The host as the open worlds and the friend list say it should be.
    fn lan_share_reconcile(&mut self, cx: &mut Context<Self>) {
        let open = self
            .lan_share
            .worlds
            .lock()
            .map(|w| w.open())
            .unwrap_or_default();
        if open != self.lan_share.open {
            self.lan_share.open = open.clone();
            cx.notify();
        }
        let (Some(identity), false) = (self.lan_share.identity.clone(), open.is_empty()) else {
            // nothing open (or not listed yet): nothing published, so friends see
            // the launcher offline
            let share = &mut self.lan_share;
            if share.host.take().is_some() {
                share.state = None;
                share.host_friends.clear();
                cx.notify();
            }
            return;
        };
        let host_name = self.lan_host_name(cx);
        let friends = self.lan_share_friends();
        let share = &mut self.lan_share;
        let keys: Vec<String> = friends.iter().map(|f| f.key.to_hex()).collect();
        match &share.host {
            Some(host) => {
                host.set_worlds(open, &host_name);
                if keys != share.host_friends {
                    host.set_friends(friends);
                    share.host_friends = keys;
                }
            }
            None => {
                let config = LanHostConfig {
                    service: friend_list::service(),
                    gather: Default::default(),
                    knocks: share.knocks.clone(),
                };
                match LanHost::start(identity, friends, open, host_name, config) {
                    Ok(host) => {
                        share.host = Some(host);
                        share.host_friends = keys;
                    }
                    Err(err) => share.problem = Some(err.to_string()),
                }
            }
        }
        let state = share.host.as_ref().map(LanHost::state);
        if state != share.state {
            share.state = state;
            cx.notify();
        }
    }

    /// The friends to share with, from the friend list's copy: accepted, not hidden.
    fn lan_share_friends(&self) -> Vec<FriendAccess> {
        let snapshot = &self.friend_list.snapshot;
        let (Some(identity), Some(me)) =
            (&self.lan_share.identity, FriendKey::from_hex(&snapshot.key))
        else {
            return Vec::new();
        };
        lan_friends(&snapshot.book, &me, &identity.fingerprint())
    }

    /// The name friends see as the world's host: the person's name in the friend list,
    /// else their Minecraft name.
    fn lan_host_name(&self, cx: &gpui::App) -> String {
        let name = &self.friend_list.snapshot.book.name;
        if !name.is_empty() {
            return name.clone();
        }
        self.my_profile(cx).map(|p| p.name).unwrap_or_default()
    }

    /// For the offscreen screenshots: sharing on, with these worlds open.
    pub fn preview_lan_share(&mut self, open: Vec<OpenWorld>, connected: usize) {
        self.lan_share.preview = true;
        self.lan_share.enabled = true;
        self.lan_share.open = open;
        self.lan_share.state = Some(LanHostState::Sharing { connected });
    }
}

/// An instance's loader as servers name it.
fn loader_name(loader: launcher_core::instances::Loader) -> &'static str {
    use launcher_core::instances::Loader;
    match loader {
        Loader::None => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
    }
}
