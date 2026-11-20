//! The Discord status (Rich Presence) while a game runs: the instance or modpack, its
//! version and loader, the time played, and where the player is: the menus, a
//! single-player world with its dimension, coordinates and stats, or a server.
//!
//! The window's thread only follows the game ([`Instances::playing`]) and hands what
//! changed to one worker thread, which does everything else: reading the world's
//! save files (only when the game saved), formatting, and talking to the Discord app.
//! The worker sleeps until something changes or a timer it set is due; with no game
//! running, or the setting off, it waits on its channel and costs nothing.
//!
//! - It connects only while a game runs, and disconnects when the last one exits,
//!   which clears the status. While Discord isn't open it tries again after 5 s,
//!   doubling to a minute.
//! - It sends the status only when it changed, and at most once every
//!   [`MIN_SEND_INTERVAL`] (Discord's limit is 5 updates in 20 s).
//! - In a world, the second line takes turns between the place, the location and the
//!   stats every [`ROTATE_EVERY`].

use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

use futures::StreamExt as _;
use gpui::{Context, Entity, Subscription, Task};
use launcher_core::{
    discord::{
        self,
        activity::{GameActivity, Place, public_server_name},
        ipc::{self, Activity, ActivityButton, Assets, Connection, Timestamps},
        world::{self, Dimension, WorldState, WorldWatcher},
    },
    instances::Loader,
    modpack::ModpackRecord,
    settings::DiscordSettings,
};
use lodestone_minecraft::level::GameMode;

use crate::{
    i18n::{ShellMsg as M, ShellPlural as P, WorldsMsg, format, t, tr, trn},
    instances::{Instances, InstancesEvent},
};

/// The shortest time between two updates of the status.
const MIN_SEND_INTERVAL: Duration = Duration::from_secs(5);
/// How long each of the second line's texts shows in a world.
const ROTATE_EVERY: Duration = Duration::from_secs(20);
/// How often an open single-player world's `level.dat` is looked at (one `stat`).
const WORLD_CHECK_EVERY: Duration = Duration::from_secs(15);
/// How long after "Saving and pausing game..." the files are looked at: the line is
/// printed before the game writes them.
const AFTER_SAVE: Duration = Duration::from_secs(2);
const FIRST_RETRY: Duration = Duration::from_secs(5);
const MAX_RETRY: Duration = Duration::from_secs(60);
/// The status's image while no modpack icon shows: an art asset of the Discord
/// application.
const APP_ASSET: &str = "lodestone";

/// What Settings > General > Discord says about the status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// This build has no Discord application id.
    Unavailable,
    /// The setting is off.
    Off,
    /// No game runs.
    #[default]
    Idle,
    /// A game runs, but the Discord app isn't open.
    NotRunning,
    /// The status shows the game.
    Connected,
}

/// The game the status shows, as the worker needs it.
#[derive(Clone, Debug, PartialEq)]
struct Game {
    id: String,
    name: String,
    game_version: String,
    loader: Loader,
    loader_version: Option<String>,
    instance_dir: PathBuf,
    game_dir: PathBuf,
    /// Unix seconds.
    started: u64,
    /// Seconds played before this game started.
    played_before: u64,
    activity: GameActivity,
}

enum Command {
    Settings(DiscordSettings),
    Game(Option<Box<Game>>),
}

/// Follows the games and keeps the worker told.
pub struct DiscordPresence {
    instances: Entity<Instances>,
    client_id: Option<String>,
    settings: DiscordSettings,
    game: Option<Game>,
    worker: Option<mpsc::Sender<Command>>,
    /// What the worker last said.
    worker_status: Status,
    _status_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl DiscordPresence {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let settings = instances.read(cx).discord_settings();
        let subscriptions = vec![
            cx.subscribe(&instances, |this, _, event, cx| match event {
                InstancesEvent::ActivityChanged(_) | InstancesEvent::GameExited(_) => this.sync(cx),
                _ => {}
            }),
            // Every redraw of the instances: only the settings are compared here.
            cx.observe(&instances, |this, instances, cx| {
                if instances.read(cx).discord_settings() != this.settings {
                    this.sync(cx);
                }
            }),
        ];
        let mut this = Self {
            instances,
            client_id: discord::client_id(),
            settings,
            game: None,
            worker: None,
            worker_status: Status::Idle,
            _status_task: None,
            _subscriptions: subscriptions,
        };
        this.sync(cx);
        this
    }

    /// What Settings shows.
    pub fn status(&self) -> Status {
        if self.client_id.is_none() {
            Status::Unavailable
        } else if !self.settings.enabled {
            Status::Off
        } else {
            self.worker_status
        }
    }

    /// Hands the worker the settings and the game, where they changed.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let instances = self.instances.read(cx);
        let settings = instances.discord_settings();
        let game = instances.playing().and_then(|(id, started, activity)| {
            let instance = instances.list().iter().find(|i| i.id == id)?;
            Some(Game {
                id: id.to_owned(),
                name: instance.name.clone(),
                game_version: instance.game_version.clone(),
                loader: instance.loader,
                loader_version: instance.loader_version.clone(),
                instance_dir: instances.content_dirs(id).instance,
                game_dir: instances.game_dir(id),
                started,
                played_before: instance.play_time,
                activity: activity.clone(),
            })
        });
        let settings_changed = settings != self.settings;
        let game_changed = game != self.game;
        self.settings = settings;
        self.game = game;
        if settings_changed {
            cx.notify();
        }
        // The worker starts with the first game the status would show.
        if self.worker.is_none() && (!settings.enabled || self.game.is_none()) {
            return;
        }
        let Some(worker) = self.worker(cx) else {
            return;
        };
        let mut sent = true;
        if settings_changed {
            sent &= worker.send(Command::Settings(settings)).is_ok();
        }
        if game_changed {
            sent &= worker
                .send(Command::Game(self.game.clone().map(Box::new)))
                .is_ok();
        }
        if !sent {
            // The worker ended (it never does unless it panicked); start another next time.
            self.worker = None;
        }
    }

    /// The worker's channel, starting it if need be. `None` without an application id.
    fn worker(&mut self, cx: &mut Context<Self>) -> Option<mpsc::Sender<Command>> {
        if let Some(worker) = &self.worker {
            return Some(worker.clone());
        }
        let client_id = self.client_id.clone()?;
        let (tx, rx) = mpsc::channel();
        let (status_tx, mut status_rx) = futures::channel::mpsc::unbounded();
        let worker = Worker::new(client_id, rx, status_tx, self.settings, self.game.clone());
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || worker.run())
            .ok()?;
        self._status_task = Some(cx.spawn(async move |this, cx| {
            while let Some(status) = status_rx.next().await {
                let alive = this.update(cx, |this, cx| {
                    this.worker_status = status;
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
        }));
        self.worker = Some(tx.clone());
        Some(tx)
    }
}

/// What the worker keeps about the game it shows.
struct Session {
    id: String,
    pack: Option<ModpackRecord>,
    /// Enabled mods in the game folder's `mods`.
    mods: usize,
    world: Option<WorldWatcher>,
    world_state: Option<WorldState>,
    /// [`GameActivity::saves`] as last seen.
    saves_seen: u64,
    world_check_at: Option<Instant>,
}

impl Session {
    fn new(game: &Game) -> Self {
        let mods = if game.loader == Loader::None {
            0
        } else {
            std::fs::read_dir(game.game_dir.join("mods"))
                .map(|dir| {
                    dir.flatten()
                        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jar"))
                        .count()
                })
                .unwrap_or(0)
        };
        Session {
            id: game.id.clone(),
            pack: ModpackRecord::load(&game.instance_dir),
            mods,
            world: None,
            world_state: None,
            saves_seen: game.activity.saves,
            world_check_at: None,
        }
    }
}

struct Worker {
    client_id: String,
    rx: mpsc::Receiver<Command>,
    status_tx: futures::channel::mpsc::UnboundedSender<Status>,
    status: Status,
    settings: DiscordSettings,
    game: Option<Game>,
    session: Option<Session>,
    connection: Option<Connection>,
    retry_at: Option<Instant>,
    backoff: Duration,
    /// What Discord shows now.
    sent: Option<Activity>,
    sent_at: Option<Instant>,
    /// When a change held back by [`MIN_SEND_INTERVAL`] goes out.
    send_at: Option<Instant>,
    rotation: usize,
    rotate_at: Option<Instant>,
    pid: u32,
}

impl Worker {
    fn new(
        client_id: String,
        rx: mpsc::Receiver<Command>,
        status_tx: futures::channel::mpsc::UnboundedSender<Status>,
        settings: DiscordSettings,
        game: Option<Game>,
    ) -> Self {
        Worker {
            client_id,
            rx,
            status_tx,
            status: Status::Idle,
            settings,
            game,
            session: None,
            connection: None,
            retry_at: None,
            backoff: FIRST_RETRY,
            sent: None,
            sent_at: None,
            send_at: None,
            rotation: 0,
            rotate_at: None,
            pid: std::process::id(),
        }
    }

    fn run(mut self) {
        loop {
            self.tick();
            let command = match self.next_deadline() {
                None => match self.rx.recv() {
                    Ok(command) => command,
                    Err(_) => return,
                },
                Some(at) => {
                    match self
                        .rx
                        .recv_timeout(at.saturating_duration_since(Instant::now()))
                    {
                        Ok(command) => command,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
            };
            self.apply(command);
            // Whatever else arrived meanwhile, so a burst makes one update.
            while let Ok(command) = self.rx.try_recv() {
                self.apply(command);
            }
        }
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::Settings(settings) => self.settings = settings,
            Command::Game(game) => self.game = game.map(|game| *game),
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let retry = self.connection.is_none().then_some(self.retry_at).flatten();
        let world = self.session.as_ref().and_then(|s| s.world_check_at);
        [retry, self.send_at, self.rotate_at, world]
            .into_iter()
            .flatten()
            .min()
    }

    fn set_status(&mut self, status: Status) {
        if self.status != status {
            self.status = status;
            let _ = self.status_tx.unbounded_send(status);
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let Some(game) = self.game.clone().filter(|_| self.settings.enabled) else {
            // Disconnecting clears the status.
            self.connection = None;
            self.session = None;
            self.sent = None;
            self.send_at = None;
            self.retry_at = None;
            self.rotate_at = None;
            self.rotation = 0;
            self.backoff = FIRST_RETRY;
            self.set_status(Status::Idle);
            return;
        };
        if self.session.as_ref().is_none_or(|s| s.id != game.id) {
            self.session = Some(Session::new(&game));
            self.rotation = 0;
            self.rotate_at = None;
        }
        self.update_world(&game, now);
        // The second line's next text, when its turn is up.
        if let Some(at) = self.rotate_at
            && now >= at
        {
            self.rotation = self.rotation.wrapping_add(1);
            self.rotate_at = Some(now + ROTATE_EVERY);
        }
        let session = self.session.as_ref().expect("made above");
        let built = build_activity(
            &game,
            &self.settings,
            session,
            self.rotation,
            launcher_core::instances::now(),
        );
        if built.states < 2 {
            self.rotate_at = None;
        } else if self.rotate_at.is_none() {
            self.rotate_at = Some(now + ROTATE_EVERY);
        }
        self.send(now, built.activity);
    }

    /// Looks at the open world's files when they are due, and reads them if the game
    /// saved since.
    fn update_world(&mut self, game: &Game, now: Instant) {
        let settings = self.settings;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let wanted = matches!(game.activity.place, Place::Singleplayer { .. })
            && (settings.show_world || settings.show_coordinates || settings.show_stats);
        if !wanted {
            session.world = None;
            session.world_state = None;
            session.world_check_at = None;
            session.saves_seen = game.activity.saves;
            return;
        }
        if game.activity.saves != session.saves_seen {
            session.saves_seen = game.activity.saves;
            let soon = now + AFTER_SAVE;
            if session.world_check_at.is_none_or(|at| at > soon) {
                session.world_check_at = Some(soon);
            }
        }
        if session.world_check_at.is_some_and(|at| now < at) {
            return;
        }
        if session.world.is_none() {
            session.world = world::open_world(&game.game_dir).map(WorldWatcher::new);
        }
        if let Some(state) = session.world.as_mut().and_then(WorldWatcher::refresh) {
            session.world_state = Some(state);
        }
        session.world_check_at = Some(now + WORLD_CHECK_EVERY);
    }

    /// Shows `activity` if Discord doesn't already, connecting first if need be.
    fn send(&mut self, now: Instant, activity: Activity) {
        if self.connection.is_none() {
            if self.retry_at.is_some_and(|at| now < at) {
                return;
            }
            match Connection::open(&self.client_id) {
                Ok(connection) => {
                    launcher_core::trace::emit("discord.connected", || serde_json::json!({}));
                    self.connection = Some(connection);
                    self.retry_at = None;
                    self.backoff = FIRST_RETRY;
                    self.sent = None;
                }
                Err(err) => {
                    launcher_core::trace::emit(
                        "discord.unavailable",
                        || serde_json::json!({ "error": err.to_string() }),
                    );
                    self.retry_at = Some(now + self.backoff);
                    self.backoff = (self.backoff * 2).min(MAX_RETRY);
                    self.set_status(Status::NotRunning);
                    return;
                }
            }
        }
        if self.sent.as_ref() == Some(&activity) {
            self.send_at = None;
            self.set_status(Status::Connected);
            return;
        }
        if let Some(at) = self.sent_at.map(|at| at + MIN_SEND_INTERVAL)
            && now < at
        {
            self.send_at = Some(at);
            return;
        }
        self.send_at = None;
        let result = self
            .connection
            .as_mut()
            .expect("connected above")
            .set_activity(self.pid, Some(&activity));
        match result {
            Ok(()) => {
                self.sent = Some(activity);
                self.sent_at = Some(now);
                self.set_status(Status::Connected);
            }
            Err(err) => {
                launcher_core::trace::emit(
                    "discord.error",
                    || serde_json::json!({ "error": err.to_string() }),
                );
                // Discord quit or refused: start over after a while.
                self.connection = None;
                self.sent = None;
                self.retry_at = Some(now + FIRST_RETRY);
                self.set_status(Status::NotRunning);
            }
        }
    }
}

/// The status, and how many texts the second line takes turns between.
struct Built {
    activity: Activity,
    states: usize,
}

/// The status for `game`. `now` is Unix seconds.
fn build_activity(
    game: &Game,
    settings: &DiscordSettings,
    session: &Session,
    rotation: usize,
    now: u64,
) -> Built {
    let pack = session
        .pack
        .as_ref()
        .filter(|pack| !pack.title.trim().is_empty());
    let world = session.world_state.as_ref();
    let player = world.and_then(|w| w.player.as_ref());
    let dimension = player.and_then(|p| p.dimension.as_ref());

    // The first line: what is played.
    let details = match pack {
        Some(pack) if !pack.version_number.trim().is_empty() => {
            format!("{} {}", pack.title.trim(), pack.version_number.trim())
        }
        Some(pack) => pack.title.trim().to_owned(),
        None => game.name.clone(),
    };

    // The second line: where, then (in a world) the location and the stats in turn.
    let place = match &game.activity.place {
        Place::Starting => t(M::DiscordStarting).to_owned(),
        Place::Menu => t(M::DiscordInMenus).to_owned(),
        Place::Singleplayer { lan } => match world.filter(|_| settings.show_world) {
            Some(world) if *lan => tr!(M::DiscordLanIn, world = world.name()),
            Some(world) if world.hardcore() => tr!(M::DiscordHardcoreIn, world = world.name()),
            Some(world) => tr!(M::DiscordSingleplayerIn, world = world.name()),
            None => t(M::DiscordSingleplayer).to_owned(),
        },
        Place::Multiplayer { host, port } => {
            match public_server_name(host, *port).filter(|_| settings.show_server) {
                Some(server) => tr!(M::DiscordMultiplayerOn, server = server),
                None => t(M::DiscordMultiplayer).to_owned(),
            }
        }
    };
    let mut states = vec![place];
    if matches!(game.activity.place, Place::Singleplayer { .. }) {
        let mut location = Vec::new();
        if settings.show_world
            && let Some(dimension) = dimension
        {
            location.push(dimension_name(dimension));
        }
        if settings.show_coordinates
            && let Some([x, y, z]) = player.and_then(|p| p.block())
        {
            location.push(format!("{x}, {y}, {z}"));
        }
        if !location.is_empty() {
            states.push(location.join(" · "));
        }
        if settings.show_stats
            && let Some(world) = world
        {
            let mut stats = Vec::new();
            if let Some(day) = world.day {
                stats.push(tr!(M::DiscordDay, day = format::number(day.max(0) as u64)));
            }
            if let Some(stat) = &world.stats {
                if let Some(deaths) = stat.deaths {
                    stats.push(trn!(P::DiscordDeaths, deaths));
                }
                if let Some(kills) = stat.mob_kills.filter(|kills| *kills > 0) {
                    stats.push(trn!(P::DiscordMobsKilled, kills));
                }
            }
            if !stats.is_empty() {
                states.push(stats.join(" · "));
            }
        }
    }
    let state = states[rotation % states.len()].clone();

    // The large image: the modpack's icon, else Lodestone's.
    let large_image = pack
        .and_then(|pack| pack.icon_url.clone())
        .filter(|url| url.starts_with("https://") && url.len() <= 256)
        .unwrap_or_else(|| APP_ASSET.to_owned());
    let mut about = vec![tr!(M::DiscordMinecraft, version = game.game_version)];
    if let Some(loader) = loader_text(game) {
        about.push(loader);
    }
    if session.mods > 0 {
        about.push(trn!(P::DiscordMods, session.mods as u64));
    }
    let played = game.played_before + now.saturating_sub(game.started);
    if played >= 60 {
        about.push(tr!(M::DiscordPlayedTotal, time = format::play_time(played)));
    }

    // The small image: the dimension in a world, else the loader.
    let (small_image, small_text) = match dimension.filter(|_| settings.show_world) {
        Some(dimension) => {
            let mut text = dimension_name(dimension);
            if let Some(mode) = world.and_then(WorldState::game_mode) {
                text = format!("{text} · {}", game_mode_name(mode));
            }
            (Some(dimension.asset().to_owned()), Some(text))
        }
        None => match loader_asset(game.loader) {
            Some(asset) => (Some(asset.to_owned()), loader_text(game)),
            None => (None, None),
        },
    };

    let buttons = pack
        .and_then(pack_page)
        .map(|url| ActivityButton {
            label: t(M::DiscordViewModpack).chars().take(32).collect(),
            url,
        })
        .into_iter()
        .collect();

    Built {
        activity: Activity {
            details: ipc::field(details),
            state: ipc::field(state),
            timestamps: Some(Timestamps {
                start: game.started.saturating_mul(1000),
            }),
            assets: Assets {
                large_image: Some(large_image),
                large_text: ipc::field(about.join(" · ")),
                small_image,
                small_text: small_text.and_then(ipc::field),
            },
            buttons,
        },
        states: states.len(),
    }
}

fn dimension_name(dimension: &Dimension) -> String {
    match dimension {
        Dimension::Overworld => t(M::DiscordOverworld).to_owned(),
        Dimension::Nether => t(M::DiscordNether).to_owned(),
        Dimension::End => t(M::DiscordEnd).to_owned(),
        Dimension::Other(id) => dimension.modded_name().unwrap_or_else(|| id.clone()),
    }
}

fn game_mode_name(mode: GameMode) -> &'static str {
    t(match mode {
        GameMode::Survival => WorldsMsg::Survival,
        GameMode::Creative => WorldsMsg::Creative,
        GameMode::Adventure => WorldsMsg::Adventure,
        GameMode::Spectator => WorldsMsg::Spectator,
    })
}

/// "Fabric 0.16.5", or `None` for vanilla.
fn loader_text(game: &Game) -> Option<String> {
    game.loader.kind()?;
    Some(match game.loader_version.as_deref().map(str::trim) {
        Some(version) if !version.is_empty() => format!("{} {version}", game.loader.label()),
        _ => game.loader.label().to_owned(),
    })
}

/// The loader's art asset in the Discord application.
fn loader_asset(loader: Loader) -> Option<&'static str> {
    Some(match loader {
        Loader::None => return None,
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
    })
}

/// The modpack's page: on Modrinth, else on CurseForge.
fn pack_page(pack: &ModpackRecord) -> Option<String> {
    let slug = pack.slug.trim();
    if !slug.is_empty()
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Some(format!("https://modrinth.com/modpack/{slug}"));
    }
    pack.curseforge
        .as_ref()
        .map(|cf| cf.page_url.trim())
        .filter(|url| url.starts_with("https://www.curseforge.com/") && url.len() <= 512)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use launcher_core::discord::world::{PlayerState, WorldStats};
    use lodestone_minecraft::level::LevelSummary;

    use super::*;

    fn game(place: Place) -> Game {
        Game {
            id: "a".into(),
            name: "My Instance".into(),
            game_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.5".into()),
            instance_dir: PathBuf::new(),
            game_dir: PathBuf::new(),
            started: 1_000,
            played_before: 3_600,
            activity: GameActivity { place, saves: 0 },
        }
    }

    fn session(world: Option<WorldState>, pack: Option<ModpackRecord>) -> Session {
        Session {
            id: "a".into(),
            pack,
            mods: 12,
            world: None,
            world_state: world,
            saves_seen: 0,
            world_check_at: None,
        }
    }

    fn world() -> WorldState {
        WorldState {
            folder: "New World".into(),
            summary: Some(LevelSummary {
                name: Some("Island".into()),
                ..Default::default()
            }),
            day: Some(12),
            player: Some(PlayerState {
                dimension: Some(Dimension::Nether),
                position: Some([10.5, 64.0, -3.2]),
                game_mode: Some(GameMode::Survival),
            }),
            stats: Some(WorldStats {
                deaths: Some(1),
                mob_kills: Some(5),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn shows_the_menus() {
        let built = build_activity(
            &game(Place::Menu),
            &DiscordSettings::default(),
            &session(None, None),
            0,
            1_060,
        );
        let activity = built.activity;
        assert_eq!(built.states, 1);
        assert_eq!(activity.details.as_deref(), Some("My Instance"));
        assert_eq!(activity.state.as_deref(), Some("In the menus"));
        assert_eq!(activity.timestamps.unwrap().start, 1_000_000);
        assert_eq!(activity.assets.large_image.as_deref(), Some(APP_ASSET));
        assert_eq!(
            activity.assets.large_text.as_deref(),
            Some("Minecraft 1.21.1 · Fabric 0.16.5 · 12 mods · 1 h 1 min played")
        );
        assert_eq!(activity.assets.small_image.as_deref(), Some("fabric"));
        assert!(activity.buttons.is_empty());
    }

    #[test]
    fn takes_turns_in_a_world_and_keeps_coordinates_private() {
        let game = game(Place::Singleplayer { lan: false });
        let session = session(Some(world()), None);
        let settings = DiscordSettings::default();
        let states: Vec<String> = (0..3)
            .map(|turn| {
                build_activity(&game, &settings, &session, turn, 1_000)
                    .activity
                    .state
                    .unwrap()
            })
            .collect();
        assert_eq!(
            states,
            [
                "Single player: Island",
                "The Nether",
                "Day 12 · 1 death · 5 mobs killed"
            ]
        );
        let built = build_activity(&game, &settings, &session, 0, 1_000);
        assert_eq!(
            built.activity.assets.small_image.as_deref(),
            Some("the_nether")
        );
        assert_eq!(
            built.activity.assets.small_text.as_deref(),
            Some("The Nether · Survival")
        );

        let coordinates = DiscordSettings {
            show_coordinates: true,
            ..settings
        };
        let built = build_activity(&game, &coordinates, &session, 1, 1_000);
        assert_eq!(
            built.activity.state.as_deref(),
            Some("The Nether · 10, 64, -4")
        );

        let nothing = DiscordSettings {
            show_world: false,
            show_stats: false,
            ..settings
        };
        let built = build_activity(&game, &nothing, &session, 0, 1_000);
        assert_eq!(built.states, 1);
        assert_eq!(built.activity.state.as_deref(), Some("Single player"));
        assert_eq!(built.activity.assets.small_image.as_deref(), Some("fabric"));
    }

    #[test]
    fn shows_modpacks_and_servers() {
        let pack = ModpackRecord {
            title: "Fabulously Optimized".into(),
            version_number: "6.1.0".into(),
            slug: "fabulously-optimized".into(),
            icon_url: Some("https://cdn.modrinth.com/data/1/icon.png".into()),
            ..Default::default()
        };
        let place = Place::Multiplayer {
            host: "play.example.com".into(),
            port: 25565,
        };
        let built = build_activity(
            &game(place),
            &DiscordSettings::default(),
            &session(None, Some(pack)),
            0,
            1_000,
        );
        let activity = built.activity;
        assert_eq!(
            activity.details.as_deref(),
            Some("Fabulously Optimized 6.1.0")
        );
        assert_eq!(
            activity.state.as_deref(),
            Some("Multiplayer: play.example.com")
        );
        assert_eq!(
            activity.assets.large_image.as_deref(),
            Some("https://cdn.modrinth.com/data/1/icon.png")
        );
        assert_eq!(
            activity.buttons,
            [ActivityButton {
                label: "View modpack".into(),
                url: "https://modrinth.com/modpack/fabulously-optimized".into()
            }]
        );

        let place = Place::Multiplayer {
            host: "192.168.0.4".into(),
            port: 25565,
        };
        let built = build_activity(
            &game(place),
            &DiscordSettings::default(),
            &session(None, None),
            0,
            1_000,
        );
        assert_eq!(built.activity.state.as_deref(), Some("Multiplayer"));
    }
}
