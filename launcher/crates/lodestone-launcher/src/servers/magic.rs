//! Magic localhost on the Lodestone Host page: an address on this computer for every
//! server, which any Minecraft client can join (see `launcher_core::cores::magic_localhost`
//! and `launcher/docs/friends.md`).
//!
//! A server on a core on this computer is at its own port. A server on another host has
//! a port saved with its core in `servers.json`, where a forwarder listens from startup
//! for as long as the launcher runs: Minecraft connects without the launcher knowing it
//! is about to, and the page's lists are only fetched while it is shown.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;

use gpui::Context;
use launcher_core::cores::{
    ServerInstance,
    friends::game_address,
    magic_localhost::{
        self, Forwarder, Forwarders, MAX_FORWARDERS, MAX_PER_CORE, MagicAddress, OnError, Target,
        magic_id,
    },
};

use super::{CoreEntry, CoreStatus, Notice, Servers, friends::port_taken_text};

/// The forwarders and what the page knows of them.
#[derive(Default)]
pub struct MagicState {
    forwarders: Forwarders,
    /// Open forwarders, by [`magic_id`].
    running: HashMap<String, Forwarder>,
    /// Forwarders being opened.
    starting: HashSet<String>,
    /// Ports that couldn't be opened at all this session (not even a free one), so
    /// they aren't tried on every refresh.
    failed: HashSet<(String, u16)>,
    /// The page said once that a saved port was in use.
    told_taken: bool,
    /// The offscreen screenshots' bound ports, by [`magic_id`]: no sockets there.
    preview_bound: HashMap<String, u16>,
}

/// Logs a connection a forwarder couldn't pass on: a stopped server is only worth a
/// debug line (Minecraft's server list asks every few seconds).
fn log_errors() -> OnError {
    Arc::new(|target: &Target, err: &io::Error| {
        if err.kind() == io::ErrorKind::ConnectionRefused {
            log::debug!("Magic localhost: {target} refused the connection (stopped?)");
        } else {
            log::warn!("Magic localhost: couldn't reach {target}: {err}");
        }
    })
}

impl Servers {
    /// The forwarders every core not on this computer should have: by [`magic_id`], the
    /// saved port and where it forwards.
    fn magic_wanted(&self) -> HashMap<String, (u16, Target)> {
        let mut wanted = HashMap::new();
        for (index, core) in self.cores.iter().enumerate() {
            if self.is_local_card(index) {
                continue;
            }
            let entries = core.saved.magic_ports.iter().filter(|e| e.is_valid());
            for entry in entries.take(MAX_PER_CORE) {
                if wanted.len() >= MAX_FORWARDERS {
                    return wanted;
                }
                if let Some(target) = Target::new(&core.url, entry.target) {
                    let id = magic_id(&core.saved.url, &entry.instance);
                    wanted.insert(id, (entry.port, target));
                }
            }
        }
        wanted
    }

    /// Opens, points and closes the forwarders to match the saved ports. Opening one
    /// probes its port (up to half a second), so they open side by side, off the
    /// window's thread. Never in the offscreen screenshots.
    pub(super) fn sync_magic(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let wanted = self.magic_wanted();
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
            if self.magic.starting.contains(&id) || self.magic.failed.contains(&(id.clone(), saved))
            {
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
        let still = self.magic_wanted().remove(&id);
        match (result, still) {
            (Ok(forwarder), Some((now, target))) if now == saved => {
                forwarder.set_target(target);
                if let Some(taken) = forwarder.taken() {
                    log::info!(
                        "Magic localhost: port {taken} was in use, so {id} is at port {} this time",
                        forwarder.port()
                    );
                    if !self.magic.told_taken {
                        self.magic.told_taken = true;
                        let address = game_address(forwarder.port());
                        self.notice = Some(Notice::info(move || port_taken_text(taken, &address)));
                    }
                }
                self.magic.running.insert(id, forwarder);
            }
            (Err(err), _) => {
                log::warn!("Magic localhost: couldn't listen for {id} (port {saved}): {err}");
                self.magic.failed.insert((id, saved));
            }
            // removed, or given another port, meanwhile: this one closes
            _ => self.sync_magic(cx),
        }
        cx.notify();
    }

    /// Core `index`'s list arrived: its servers get their ports (saved when they
    /// change), and the forwarders follow.
    pub(super) fn magic_listed(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.is_local_card(index) {
            let taken = self.magic_taken(Some(index));
            let core = &mut self.cores[index];
            let authoritative = core.signed_in();
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

    /// The ports Magic localhost ports stay away from: every core's but `except`'s, and
    /// friends' servers' game ports.
    fn magic_taken(&self, except: Option<usize>) -> Vec<u16> {
        let mut taken = self.host_magic_ports_except(except);
        taken.extend(self.friends.list.iter().filter_map(|f| f.saved.game_port));
        taken
    }

    fn host_magic_ports_except(&self, except: Option<usize>) -> Vec<u16> {
        self.cores
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != except)
            .flat_map(|(_, core)| core.saved.magic_ports.iter().map(|e| e.port))
            .collect()
    }

    /// Every core's Magic localhost ports, for friends' servers' ports to stay away
    /// from.
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
        magic_localhost::magic_address(
            self.is_local_card(index),
            &core.saved.magic_ports,
            server,
            bound,
        )
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
            self.notice = Some(Notice::info(move || port_taken_text(taken, &address)));
        }
    }
}
