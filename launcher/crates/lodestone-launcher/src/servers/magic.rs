//! Magic localhost on the Lodestone Host page: an address on this computer for every
//! server, which any Minecraft client can join (see `launcher_core::cores::magic_localhost`
//! and `launcher/docs/friends.md`).
//!
//! A server on a core on this computer is at its own port. A server on another host has
//! a port saved with its core in `servers.json`, where a forwarder listens from startup
//! for as long as the launcher runs: Minecraft connects without the launcher knowing it
//! is about to, and the page's lists are only fetched while it is shown.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::Context;
use launcher_core::cores::{
    ServerInstance,
    friends::game_address,
    magic_localhost::{
        self, Forwarder, Forwarders, HostCore, MagicAddress, OnError, Target, magic_id,
    },
};

use super::{CoreEntry, CoreStatus, Notice, Servers, friends::port_taken_text};
use crate::i18n::ServersMsg as PortMsg;

/// The forwarders and what the page knows of them.
#[derive(Default)]
pub struct MagicState {
    forwarders: Forwarders,
    /// Open forwarders, by [`magic_id`].
    running: HashMap<String, Forwarder>,
    /// Forwarders being opened.
    starting: HashSet<String>,
    /// Saved ports, by [`magic_id`], that couldn't be opened at all (not even a free
    /// one instead), and when, so they aren't tried on every refresh: again after
    /// [`RETRY_AFTER`], as the cause usually passes (out of files for a moment, say).
    /// Their rows show no address meanwhile: nothing would answer it.
    failed: HashMap<String, (u16, Instant)>,
    /// Servers, by [`magic_id`], with a saved port but no forwarder: past their core's
    /// share of [`magic_localhost::MAX_FORWARDERS`], or a host that turned out to be
    /// this computer. Their rows show no address either.
    unserved: HashSet<String>,
    /// The page said once that a saved port was in use.
    told_taken: bool,
    /// The offscreen screenshots' bound ports, by [`magic_id`]: no sockets there.
    preview_bound: HashMap<String, u16>,
}

/// How long a forwarder that couldn't listen waits before the next list tries again.
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// How often a forwarder warns about one target it can't reach. Minecraft's server
/// list asks every few seconds while it's open, so the rest are debug lines.
const WARN_EVERY: Duration = Duration::from_secs(60);

/// Logs a connection a forwarder couldn't pass on: a stopped server is only worth a
/// debug line, and any other failure a warning at most once a [`WARN_EVERY`] for each
/// target.
fn log_errors() -> OnError {
    let warned: Mutex<HashMap<String, Instant>> = Mutex::default();
    Arc::new(move |target: &Target, err: &io::Error| {
        if err.kind() == io::ErrorKind::ConnectionRefused {
            log::debug!("Magic localhost: {target} refused the connection (stopped?)");
            return;
        }
        let now = Instant::now();
        let mut warned = warned.lock().unwrap_or_else(|e| e.into_inner());
        // a forwarder has one target at a time, so this stays small
        warned.retain(|_, at| now.duration_since(*at) < WARN_EVERY);
        match warned.entry(target.to_string()) {
            Entry::Occupied(_) => log::debug!("Magic localhost: couldn't reach {target}: {err}"),
            Entry::Vacant(entry) => {
                entry.insert(now);
                log::warn!("Magic localhost: couldn't reach {target}: {err}");
            }
        }
    })
}

impl Servers {
    /// The forwarders every core not on this computer should have: by [`magic_id`], the
    /// saved port and where it forwards. Also the servers with a saved port that get
    /// none. [`magic_localhost::wanted_forwarders`] shares them fairly among the cores.
    fn magic_wanted(&self) -> (HashMap<String, (u16, Target)>, HashSet<String>) {
        let cores: Vec<HostCore> = (0..self.cores.len())
            .filter(|&index| !self.is_local_card(index))
            .map(|index| {
                let core = &self.cores[index];
                HostCore {
                    saved_url: &core.saved.url,
                    url: &core.url,
                    saved: &core.saved.magic_ports,
                    listed: match &core.status {
                        CoreStatus::Online(list) => Some(list.instances.as_slice()),
                        _ => None,
                    },
                }
            })
            .collect();
        let (wanted, unserved) = magic_localhost::wanted_forwarders(&cores);
        let wanted = wanted
            .into_iter()
            .map(|w| (w.id, (w.port, w.target)))
            .collect();
        (wanted, unserved.into_iter().collect())
    }

    /// Opens, points and closes the forwarders to match the saved ports. Opening one
    /// probes its port (up to half a second), so they open side by side, off the
    /// window's thread. Never in the offscreen screenshots.
    pub(super) fn sync_magic(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        self.forget_local_magic();
        self.release_local_ports();
        let (wanted, unserved) = self.magic_wanted();
        self.magic.unserved = unserved;
        self.magic.running.retain(|id, forwarder| {
            wanted
                .get(id)
                .is_some_and(|(saved, _)| *saved == forwarder.saved())
        });
        self.tasks.retain(|task| !task.is_ready());
        for (id, (saved, target)) in wanted {
            if let Some(forwarder) = self.magic.running.get(&id) {
                forwarder.set_target(target);
                continue;
            }
            let failed_lately = self
                .magic
                .failed
                .get(&id)
                .is_some_and(|(port, at)| *port == saved && at.elapsed() < RETRY_AFTER);
            if self.magic.starting.contains(&id) || failed_lately {
                continue;
            }
            self.magic.starting.insert(id.clone());
            let start = self.magic.forwarders.start(saved, target, log_errors());
            self.tasks.push(cx.spawn(async move |this, cx| {
                let result = start.await;
                this.update(cx, |this, cx| this.magic_started(id, saved, result, cx))
                    .ok();
            }));
        }
    }

    fn magic_started(
        &mut self,
        id: String,
        saved: u16,
        result: io::Result<Forwarder>,
        cx: &mut Context<Self>,
    ) {
        self.magic.starting.remove(&id);
        let still = self.magic_wanted().0.remove(&id);
        match (result, still) {
            (Ok(forwarder), Some((now, target))) if now == saved => {
                self.magic.failed.remove(&id);
                forwarder.set_target(target);
                if let Some(taken) = forwarder.taken() {
                    log::info!(
                        "Magic localhost: port {taken} was in use, so {id} is at port {} this time",
                        forwarder.port()
                    );
                    if !self.magic.told_taken {
                        self.magic.told_taken = true;
                        let address = game_address(forwarder.port());
                        self.notice = Some(port_taken_notice(taken, address));
                    }
                }
                self.magic.running.insert(id, forwarder);
            }
            (Err(err), _) => {
                log::warn!("Magic localhost: couldn't listen for {id} (port {saved}): {err}");
                self.magic.failed.insert(id, (saved, Instant::now()));
            }
            // removed, or given another port, meanwhile: this one closes
            _ => self.sync_magic(cx),
        }
        cx.notify();
    }

    /// Core `index`'s list arrived: its servers get their ports (saved when they
    /// change), and the forwarders follow. `complete`: the list has every server the
    /// core shows this device (signed in, and its public half arrived too), so servers
    /// missing from it lose their ports.
    pub(super) fn magic_listed(&mut self, index: usize, complete: bool, cx: &mut Context<Self>) {
        if !self.is_local_card(index) {
            let taken = self.magic_taken(Some(index));
            let core = &mut self.cores[index];
            let authoritative = complete && core.signed_in();
            let changed = match &core.status {
                CoreStatus::Online(list) => magic_localhost::update_ports(
                    &mut core.saved.magic_ports,
                    &core.saved.url,
                    &list.instances,
                    authoritative,
                    &taken,
                ),
                _ => false,
            };
            if changed {
                self.save();
            }
        }
        self.sync_magic(cx);
    }

    /// A core's card that moved to "On this computer" (the scan found it here, at an
    /// address that first looked like another host's) needs no ports: its servers are
    /// at their own. Dropping them frees the range and closes their forwarders.
    fn forget_local_magic(&mut self) {
        let local: Vec<usize> = (0..self.cores.len())
            .filter(|&index| {
                self.is_local_card(index) && !self.cores[index].saved.magic_ports.is_empty()
            })
            .collect();
        for &index in &local {
            self.cores[index].saved.magic_ports.clear();
        }
        if !local.is_empty() {
            self.save();
        }
    }

    /// A saved port that is now the game port of a server on this computer (a server
    /// made or changed after the port was picked) would answer that server's row's
    /// address instead of the server: Java's server socket shares the port, and the
    /// forwarder's 127.0.0.1 bind is the more specific one. Such ports go, and their
    /// servers get new ones away from it, from the core's last list.
    fn release_local_ports(&mut self) {
        let local = self.local_game_ports();
        if local.is_empty() {
            return;
        }
        let mut changed = false;
        for index in 0..self.cores.len() {
            if self.is_local_card(index)
                || !magic_localhost::release(&mut self.cores[index].saved.magic_ports, &local)
            {
                continue;
            }
            log::info!(
                "Magic localhost: a server on this computer took a port of {}'s, so its server gets another",
                self.cores[index].saved.url
            );
            changed = true;
            let taken = self.magic_taken(Some(index));
            let core = &mut self.cores[index];
            if let CoreStatus::Online(list) = &core.status {
                magic_localhost::update_ports(
                    &mut core.saved.magic_ports,
                    &core.saved.url,
                    &list.instances,
                    false,
                    &taken,
                );
            }
        }
        if changed {
            self.save();
        }
    }

    /// The game ports of the servers on this computer's cores, which their rows show as
    /// their Magic localhost.
    fn local_game_ports(&self) -> Vec<u16> {
        (0..self.cores.len())
            .filter(|&index| self.is_local_card(index))
            .flat_map(|index| self.cores[index].servers().iter())
            .filter_map(|s| u16::try_from(s.port?).ok())
            .collect()
    }

    /// The ports Magic localhost ports stay away from: every core's but `except`'s, and
    /// friends' servers' game ports.
    fn magic_taken(&self, except: Option<usize>) -> Vec<u16> {
        let mut taken = self.host_magic_ports_except(except);
        taken.extend(self.friends.list.iter().filter_map(|f| f.saved.game_port));
        taken
    }

    /// Every core's Magic localhost ports but `except`'s, and the game ports of the
    /// servers on this computer's cores: their rows show `127.0.0.1:<that port>`, which
    /// a forwarder there would answer instead (the more specific bind wins).
    fn host_magic_ports_except(&self, except: Option<usize>) -> Vec<u16> {
        let mut ports: Vec<u16> = self
            .cores
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != except)
            .flat_map(|(_, core)| core.saved.magic_ports.iter().map(|e| e.port))
            .collect();
        ports.extend(self.local_game_ports());
        ports
    }

    /// Every core's Magic localhost ports (and this computer's servers' game ports), for
    /// friends' servers' ports to stay away from.
    pub(super) fn host_magic_ports(&self) -> Vec<u16> {
        self.host_magic_ports_except(None)
    }

    /// The Magic localhost address `server`'s row on `core`'s card shows.
    pub fn magic_address(&self, core: &CoreEntry, server: &ServerInstance) -> Option<MagicAddress> {
        let index = self.core_index(&core.saved.uuid)?;
        let id = magic_id(&core.saved.url, &server.uuid);
        let bound = match self.magic.running.get(&id) {
            Some(forwarder) => Some(forwarder.port()),
            None => self.magic.preview_bound.get(&id).copied(),
        };
        let address = magic_localhost::magic_address(
            self.is_local_card(index),
            &core.saved.magic_ports,
            server,
            bound,
        )?;
        // no forwarder will listen there this session: copying it would only fail
        let dead = matches!(address, MagicAddress::Forwarded { .. })
            && bound.is_none()
            && (self.magic.unserved.contains(&id)
                || self
                    .magic
                    .failed
                    .get(&id)
                    .is_some_and(|(port, _)| *port == address.port()));
        (!dead).then_some(address)
    }

    /// The offscreen screenshots: the forwarder of server `instance` on core `core`
    /// listens on `port`, which says the saved port was taken when it isn't that one.
    pub fn preview_magic_bound(&mut self, core: &str, instance: &str, port: u16) {
        let Some(index) = self.core_index(core) else {
            return;
        };
        let saved = &self.cores[index].saved;
        let id = magic_id(&saved.url, instance);
        let taken = saved
            .magic_ports
            .iter()
            .find(|e| e.instance == instance)
            .map(|e| e.port)
            .filter(|saved| *saved != port);
        self.magic.preview_bound.insert(id, port);
        if let Some(taken) = taken {
            let address = game_address(port);
            self.notice = Some(port_taken_notice(taken, address));
        }
    }
}

/// The notice that a Magic localhost port was taken: the title and text, then the
/// address to use this time and the usual port, as values to copy.
fn port_taken_notice(saved: u16, address: String) -> Notice {
    Notice::info(port_taken_text)
        .value(
            || crate::i18n::t(PortMsg::AddressNow).into(),
            address,
            crate::components::ValueKind::Code,
        )
        .value(
            || crate::i18n::t(PortMsg::UsualPort).into(),
            saved.to_string(),
            crate::components::ValueKind::Code,
        )
}
