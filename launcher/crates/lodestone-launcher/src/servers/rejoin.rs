//! Rejoining a server that restarted while its game played on it.
//!
//! A game that Play started on a server is watched while it runs: the server's state
//! is read every few seconds. When the server stops (a host restarting it after
//! changing its mods, say) and then runs again, the launcher closes the game, brings
//! its mods in step with the server's without asking (a host not trusted for mods still
//! shows its advisory), starts it again and joins. Minecraft can't rejoin a server by
//! itself, and a server whose mods changed needs the game restarted anyway.
//!
//! A server that doesn't come back within [`COME_BACK_WITHIN`] is left alone, and
//! "Don't rejoin" on the instance page stops waiting for it.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use gpui::{AppContext, Context, Task};
use launcher_core::cores;

use super::{Servers, friends::Liveness};

/// How often the state of a server a game plays on is read.
const POLL: Duration = Duration::from_secs(4);

/// How long the launcher waits for a server that stopped to run again.
const COME_BACK_WITHIN: Duration = Duration::from_secs(10 * 60);

/// How long rejoining may take, from closing the game to starting it again (mods to
/// download, a dialog to answer).
const REJOIN_WITHIN: Duration = Duration::from_secs(10 * 60);

/// Where a watched game is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejoinPhase {
    /// The game plays on the server.
    Playing,
    /// The server stopped while the game played on it: waiting for it to run again.
    Waiting,
    /// The server runs again: the game is closing, or getting its mods and starting.
    Rejoining,
}

struct Watch {
    core: String,
    server: String,
    phase: RejoinPhase,
    /// When the phase began.
    since: Instant,
    /// The server was seen running since the game started, so a stop seen next is a
    /// real one (not a state listed before the game joined).
    seen_running: bool,
}

/// The games watched for their server restarting.
#[derive(Default)]
pub struct Rejoin {
    /// By instance id.
    watches: HashMap<String, Watch>,
    poller: Option<Task<()>>,
    /// Friends' servers whose state is being read through their tunnel.
    reading: HashSet<String>,
}

/// What a watch does next.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Stay,
    /// The server stopped: wait for it.
    Wait,
    /// The server runs again: close the game and join again.
    Rejoin,
    /// Stop watching: the game was closed, or the server didn't come back.
    Forget,
}

/// The next step of a watch in `phase`, now that the server is `live` and the game
/// is `running` (or still starting), `waited` after the phase began.
fn step(
    phase: RejoinPhase,
    seen_running: bool,
    live: Liveness,
    running: bool,
    waited: Duration,
) -> Step {
    match phase {
        RejoinPhase::Playing if !running => Step::Forget,
        RejoinPhase::Playing => match live {
            Liveness::Stopped | Liveness::Starting if seen_running => Step::Wait,
            _ => Step::Stay,
        },
        RejoinPhase::Waiting if live == Liveness::Running => Step::Rejoin,
        RejoinPhase::Waiting if waited >= COME_BACK_WITHIN => Step::Forget,
        RejoinPhase::Waiting => Step::Stay,
        // the game's exit and Play move it on; this only ends one that got stuck
        RejoinPhase::Rejoining if waited >= REJOIN_WITHIN => Step::Forget,
        RejoinPhase::Rejoining => Step::Stay,
    }
}

impl Servers {
    /// Watches the game `instance_id` starts joining `server` on `core`, to rejoin it
    /// after it restarts.
    pub(super) fn watch_for_restart(
        &mut self,
        instance_id: &str,
        core: &str,
        server: &str,
        cx: &mut Context<Self>,
    ) {
        if self.preview {
            return;
        }
        self.rejoin.watches.insert(
            instance_id.to_string(),
            Watch {
                core: core.to_string(),
                server: server.to_string(),
                phase: RejoinPhase::Playing,
                since: Instant::now(),
                seen_running: false,
            },
        );
        if self.rejoin.poller.is_some() {
            return;
        }
        self.rejoin.poller = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let watching = this
                    .update(cx, |this, cx| {
                        this.rejoin_tick(cx);
                        !this.rejoin.watches.is_empty()
                    })
                    .unwrap_or(false);
                if !watching {
                    break;
                }
            }
            this.update(cx, |this, _| this.rejoin.poller = None).ok();
        }));
    }

    /// Where the game of `instance_id` is in rejoining its server, once its server
    /// stopped: `None` while it plays as usual, or isn't watched.
    pub fn rejoin_phase(&self, instance_id: &str) -> Option<RejoinPhase> {
        self.rejoin
            .watches
            .get(instance_id)
            .map(|w| w.phase)
            .filter(|phase| *phase != RejoinPhase::Playing)
    }

    /// Whether Play for `instance_id` is rejoining its server, so a change to its mods
    /// is applied without asking.
    pub(super) fn rejoining(&self, instance_id: &str) -> bool {
        self.rejoin_phase(instance_id) == Some(RejoinPhase::Rejoining)
    }

    /// "Don't rejoin": stops waiting for the server of `instance_id`.
    pub fn cancel_rejoin(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        if self.rejoin.watches.remove(instance_id).is_some() {
            cx.notify();
        }
    }

    /// A game exited: one closed to rejoin its server starts again.
    pub(super) fn rejoin_game_exited(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        if !self.rejoining(instance_id) {
            return;
        }
        self.relaunch(instance_id, cx);
    }

    /// Reads the watched servers' states, and moves each watch on from what was read
    /// last time.
    fn rejoin_tick(&mut self, cx: &mut Context<Self>) {
        let watched: Vec<(String, String, String)> = self
            .rejoin
            .watches
            .iter()
            .map(|(instance, w)| (instance.clone(), w.core.clone(), w.server.clone()))
            .collect();
        let mut changed = false;
        for (instance_id, core, server) in watched {
            self.read_state(&core, cx);
            let live = self
                .instance_link(&instance_id)
                .filter(|link| link.core_uuid == core && link.server_uuid == server)
                .map_or_else(|| self.friend_liveness(&core), |link| link.live);
            let running = self.instances.read(cx).is_active(&instance_id);
            let Some(watch) = self.rejoin.watches.get_mut(&instance_id) else {
                continue;
            };
            if live == Liveness::Running {
                watch.seen_running = true;
            }
            match step(
                watch.phase,
                watch.seen_running,
                live,
                running,
                watch.since.elapsed(),
            ) {
                Step::Stay => {}
                Step::Wait => {
                    watch.phase = RejoinPhase::Waiting;
                    watch.since = Instant::now();
                    changed = true;
                    launcher_core::trace::emit(
                        "rejoin.wait",
                        || serde_json::json!({ "instance": instance_id }),
                    );
                }
                Step::Rejoin => {
                    watch.phase = RejoinPhase::Rejoining;
                    watch.since = Instant::now();
                    changed = true;
                    launcher_core::trace::emit(
                        "rejoin.back",
                        || serde_json::json!({ "instance": instance_id, "running": running }),
                    );
                    if running {
                        // its exit starts it again (see `rejoin_game_exited`)
                        self.instances
                            .update(cx, |instances, cx| instances.stop(&instance_id, cx));
                    } else {
                        self.relaunch(&instance_id, cx);
                    }
                }
                Step::Forget => {
                    self.rejoin.watches.remove(&instance_id);
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    /// Asks the server's host for its state again: a core's list, or a friend's server
    /// through the tunnel its game uses (asked anew once the game is closed).
    fn read_state(&mut self, core: &str, cx: &mut Context<Self>) {
        if !cores::friends::is_friend_id(core) {
            if let Some(index) = self.core_index(core) {
                self.refresh(index, cx);
            }
            return;
        }
        let Some((client, server)) = self.session_client(core) else {
            self.recheck_friends(vec![core.to_string()], cx);
            return;
        };
        if !self.rejoin.reading.insert(core.to_string()) {
            return;
        }
        let id = core.to_string();
        let fetch = cx.background_spawn(async move { client.public_servers().await });
        self.tasks.retain(|t| !t.is_ready());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let listed = fetch.await;
            this.update(cx, |this, cx| {
                this.rejoin.reading.remove(&id);
                let live = listed
                    .ok()
                    .and_then(|list| list.instances.into_iter().find(|s| s.uuid == server));
                if let Some(live) = live {
                    this.session_state(&id, &live, cx);
                }
            })
            .ok();
        }));
    }

    /// Shows the game of `instance_id` waiting for its server to come back, for the
    /// offscreen screenshots.
    pub fn preview_rejoin(&mut self, instance_id: &str, core: &str, server: &str) {
        self.rejoin.watches.insert(
            instance_id.to_string(),
            Watch {
                core: core.to_string(),
                server: server.to_string(),
                phase: RejoinPhase::Waiting,
                since: Instant::now(),
                seen_running: true,
            },
        );
    }

    /// Plays the instance again, joining its server: the friend's tunnel the old game
    /// used is closed first, so Play opens a new one.
    fn relaunch(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        let Some(core) = self.rejoin.watches.get(instance_id).map(|w| w.core.clone()) else {
            return;
        };
        self.friend_games_changed(cx);
        launcher_core::trace::emit(
            "rejoin.play",
            || serde_json::json!({ "instance": instance_id }),
        );
        if self.instance_links.contains_key(instance_id) {
            self.play_instance(instance_id, cx);
        } else if cores::friends::is_friend_id(&core) {
            // played with an instance that matched but isn't linked: the friend's
            // server remembers it
            self.play_friend(&core, cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Duration = Duration::ZERO;

    #[test]
    fn a_server_that_stops_after_running_is_waited_for() {
        assert_eq!(
            step(RejoinPhase::Playing, true, Liveness::Stopped, true, NOW),
            Step::Wait
        );
        assert_eq!(
            step(RejoinPhase::Playing, true, Liveness::Starting, true, NOW),
            Step::Wait
        );
    }

    #[test]
    fn a_stop_listed_before_the_server_was_seen_running_is_ignored() {
        // the core's list from before Play started the server
        assert_eq!(
            step(RejoinPhase::Playing, false, Liveness::Stopped, true, NOW),
            Step::Stay
        );
    }

    #[test]
    fn a_host_that_doesnt_answer_isnt_a_restart() {
        assert_eq!(
            step(RejoinPhase::Playing, true, Liveness::Unreachable, true, NOW),
            Step::Stay
        );
    }

    #[test]
    fn closing_the_game_while_playing_stops_watching() {
        assert_eq!(
            step(RejoinPhase::Playing, true, Liveness::Running, false, NOW),
            Step::Forget
        );
    }

    #[test]
    fn a_server_running_again_is_rejoined_with_or_without_the_game() {
        for running in [true, false] {
            assert_eq!(
                step(RejoinPhase::Waiting, true, Liveness::Running, running, NOW),
                Step::Rejoin
            );
        }
    }

    #[test]
    fn a_server_that_doesnt_come_back_is_left_alone() {
        assert_eq!(
            step(
                RejoinPhase::Waiting,
                true,
                Liveness::Stopped,
                true,
                COME_BACK_WITHIN - Duration::from_secs(1)
            ),
            Step::Stay
        );
        assert_eq!(
            step(
                RejoinPhase::Waiting,
                true,
                Liveness::Stopped,
                true,
                COME_BACK_WITHIN
            ),
            Step::Forget
        );
    }

    #[test]
    fn rejoining_waits_for_the_game_to_exit() {
        assert_eq!(
            step(RejoinPhase::Rejoining, true, Liveness::Running, true, NOW),
            Step::Stay
        );
        assert_eq!(
            step(
                RejoinPhase::Rejoining,
                true,
                Liveness::Running,
                false,
                REJOIN_WITHIN
            ),
            Step::Forget
        );
    }
}
