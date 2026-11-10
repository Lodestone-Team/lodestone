//! The Servers page: Lodestone cores and their servers, with "Create matching
//! instance", Play (join), Start, "Start and join" and Stop; and the same Play for an
//! instance linked to a server, from the instance page.
//!
//! The cores are kept in `servers.json` (see `launcher_core::cores::store`), each
//! device key in the Keychain. The lists are polled every few seconds while the page
//! is visible.
//!
//! Two kinds of trust are asked for:
//!
//! - a core's HTTPS certificate that macOS doesn't trust is shown with its fingerprint
//!   and pinned once the user trusts it ([`CertPrompt`], `cores::tls`);
//! - before mods or other code is installed from a core the user doesn't trust for mods,
//!   an advisory lists what will be installed ([`AdvisoryPrompt`], `cores::trust`).
//!   The user is asked whether they trust a core right after adding it, and can change it
//!   on the core's card.

pub mod add_dialog;
pub mod detail;
mod detail_view;
pub mod extras;
pub mod friends;
mod friends_view;
pub mod host;
mod host_offer;
pub mod lan;
pub mod local;
pub mod view;
// What the control API (`crate::control`) does with cores and servers.
pub(crate) mod control;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, SharedString, Subscription,
    Task,
};
use http_client::HttpClient;
use launcher_core::{
    Secret,
    content::install::InstallProgress,
    cores::{
        self, CoreClient, CoreError, PackFetch, ServerInstance, ServerList, ServerLoader,
        ServerState,
        client::display_address,
        join::join_target,
        link::{self, ServerLink},
        store::{self, SavedCore, ServersFile},
        sync::{self, PackChanges},
        tls::{CertificatePin, PinnedHttp},
        trust::{self, Advisory, PullKind, PulledFile},
    },
    game::{download::CancelToken, loaders::LoaderChoice},
    instances::Loader,
    modpack::{
        self, ModpackError, PackInfo,
        import::{self, ImportOptions},
        update::{self, UpdatePlan},
    },
};
use url::Url;

use crate::{
    accounts::Accounts,
    components::DialogSlot,
    i18n::{CommonMsg, ServersMsg as M, ServersPlural as P, format, t, tr, trn},
    instances::Instances,
};
use add_dialog::{AddRequest, AddServerDialog, AddServerEvent};

/// How often the lists are fetched while the page is visible.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// How long "Start and join" waits for the server.
pub const START_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// What trusting a core for mods means, shown wherever the user is asked.
pub fn trust_explanation() -> String {
    tr!(M::TrustExplanation, computer = t(CommonMsg::ThisComputer))
}

/// A server's state, in a word or two.
pub fn state_label(state: ServerState) -> &'static str {
    t(match state {
        ServerState::Starting => M::StateStarting,
        ServerState::Running => M::StateRunning,
        ServerState::Stopping => M::StateStopping,
        ServerState::Stopped => M::StateStopped,
        ServerState::Error => M::StateCrashed,
        ServerState::Unknown => M::StateUnknown,
        ServerState::Hidden => M::StateHidden,
    })
}

/// The players online, "3 / 20 players", when the core says.
pub fn players_label(server: &ServerInstance) -> Option<String> {
    let count = server.player_count?;
    Some(match server.max_player_count {
        Some(max) => tr!(
            M::PlayersOfMax,
            count = format::number(count.into()),
            max = format::number(max.into())
        ),
        None => trn!(P::Players, count),
    })
}

/// The state, with the players when the server runs: "Running · 3 / 20 players".
pub fn status_label(server: &ServerInstance) -> String {
    match players_label(server) {
        Some(players) if server.state == ServerState::Running => {
            format!("{} · {players}", state_label(server.state))
        }
        _ => state_label(server.state).to_string(),
    }
}

/// A server's loader: its name, or "Other".
pub fn loader_label(loader: ServerLoader) -> &'static str {
    match loader {
        ServerLoader::Other => t(M::LoaderOther),
        loader => loader.label(),
    }
}

/// Why a request to a core failed, for people. The text a core sends (and the
/// system's own errors) stays as it is.
pub fn core_error(err: &CoreError) -> String {
    // the texts launcher-core gives these errors when the core sends none
    const FORBIDDEN: &str = "The core refused: you don't have permission to do that.";
    const NOT_FOUND: &str = "Not found";
    const TOO_MANY: &str = "Too many attempts. Try again in a few minutes.";
    match err {
        CoreError::InvalidAddress(message) => message.clone(),
        CoreError::Unreachable(detail) => tr!(M::CoreUnreachable, detail = detail),
        CoreError::Untrusted { host, .. } => tr!(
            M::CoreUntrusted,
            host = host,
            computer = t(CommonMsg::ThisComputer)
        ),
        CoreError::CertificateChanged { host, .. } => tr!(M::CoreCertificateChanged, host = host),
        CoreError::SignedOut => t(M::CoreSignedOut).into(),
        CoreError::BadCredentials => t(M::CoreBadCredentials).into(),
        CoreError::Forbidden(message) if message == FORBIDDEN => t(M::CoreForbidden).into(),
        CoreError::NotFound(message) if message == NOT_FOUND => t(M::CoreNotFound).into(),
        CoreError::TooManyRequests(message) if message == TOO_MANY => {
            t(M::CoreTooManyRequests).into()
        }
        CoreError::Forbidden(message)
        | CoreError::NotFound(message)
        | CoreError::TooManyRequests(message) => message.clone(),
        CoreError::TooOld => t(M::CoreTooOld).into(),
        CoreError::Server { status, message } => {
            tr!(M::CoreAnswered, status = status, message = message)
        }
        CoreError::Invalid(detail) => tr!(M::CoreInvalidReply, detail = detail),
    }
}

/// Where to find a core's certificate fingerprint, in English, for the control API's
/// answers (the interface shows `ServersMsg::FingerprintHelp` in its language).
pub const FINGERPRINT_HELP: &str = M::FingerprintHelp.english();

/// What a notice says: something went wrong or needs a look, something worked, or
/// only what is going on. It decides the notice's color and icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticeKind {
    #[default]
    Warning,
    Success,
    Info,
}

/// A message at the top of the page: its title and text, made each time it is shown so
/// it follows the language.
#[derive(Clone)]
pub struct Notice {
    text: Rc<dyn Fn() -> (String, String)>,
    pub kind: NoticeKind,
}

impl Notice {
    /// A warning: a failure, or something to look at.
    pub fn new(text: impl Fn() -> (String, String) + 'static) -> Self {
        Self {
            text: Rc::new(text),
            kind: NoticeKind::Warning,
        }
    }

    /// Something that worked, such as an instance that is ready.
    pub fn success(text: impl Fn() -> (String, String) + 'static) -> Self {
        Self {
            kind: NoticeKind::Success,
            ..Self::new(text)
        }
    }

    /// Only what is going on, or what to know.
    pub fn info(text: impl Fn() -> (String, String) + 'static) -> Self {
        Self {
            kind: NoticeKind::Info,
            ..Self::new(text)
        }
    }

    /// The title and the text, in the current language.
    pub fn text(&self) -> (String, String) {
        (self.text)()
    }
}

/// A core's list, as last fetched.
#[derive(Clone, Debug)]
pub enum CoreStatus {
    Loading,
    Online(ServerList),
    Failed(CoreError),
}

/// A core on the page.
pub struct CoreEntry {
    pub saved: SavedCore,
    pub url: Url,
    key: Option<Secret>,
    pub status: CoreStatus,
    refreshing: bool,
    /// The client that accepts the pinned certificate.
    pinned: Option<Arc<PinnedHttp>>,
}

impl CoreEntry {
    fn new(saved: SavedCore, url: Url, key: Option<Secret>) -> Self {
        let pinned = pinned_http(saved.certificate.as_ref());
        Self {
            saved,
            url,
            key,
            status: CoreStatus::Loading,
            refreshing: false,
            pinned,
        }
    }

    pub fn signed_in(&self) -> bool {
        self.key.is_some()
    }

    pub fn servers(&self) -> &[ServerInstance] {
        match &self.status {
            CoreStatus::Online(list) => &list.instances,
            _ => &[],
        }
    }
}

fn pinned_http(pin: Option<&CertificatePin>) -> Option<Arc<PinnedHttp>> {
    PinnedHttp::new(Some(pin?.clone())).ok().map(Arc::new)
}

/// What a server row (or a linked instance's Play) is doing.
#[derive(Clone, Debug)]
pub enum Busy {
    /// Downloading the server's pack.
    Fetching,
    Creating(Arc<InstallProgress>),
    CheckingMods,
    UpdatingMods(Arc<InstallProgress>),
    Starting,
    Stopping,
}

impl Busy {
    pub fn label(&self) -> String {
        let files = |progress: &InstallProgress, plain: M, counted: M| {
            let snapshot = progress.downloads.snapshot();
            if snapshot.files_total == 0 {
                t(plain).to_string()
            } else {
                tr!(
                    counted,
                    done = format::number(snapshot.files_done),
                    total = format::number(snapshot.files_total)
                )
            }
        };
        match self {
            Busy::Fetching => t(M::BusyFetching).into(),
            Busy::Creating(progress) => files(progress, M::BusyCreating, M::BusyCreatingFiles),
            Busy::CheckingMods => t(M::BusyChecking).into(),
            Busy::UpdatingMods(progress) => files(progress, M::BusyUpdating, M::BusyUpdatingFiles),
            Busy::Starting => t(M::BusyStarting).into(),
            Busy::Stopping => t(M::BusyStopping).into(),
        }
    }
}

/// A server's pack that was downloaded and read, before the instance is made.
pub struct PendingCreate {
    core: String,
    server: ServerInstance,
    pack_path: PathBuf,
    pack: Arc<PackInfo>,
    etag: Option<String>,
    loader: Option<LoaderChoice>,
}

/// An update that brings an instance's mods in step with its server's pack.
pub struct PendingSync {
    core: String,
    server: ServerInstance,
    instance_id: String,
    pack_path: PathBuf,
    plan: Arc<UpdatePlan>,
    etag: Option<String>,
}

/// An install from a core that waits for the user.
pub enum PendingPull {
    Create(PendingCreate),
    Sync(PendingSync),
}

impl PendingPull {
    fn core(&self) -> &str {
        match self {
            PendingPull::Create(p) => &p.core,
            PendingPull::Sync(p) => &p.core,
        }
    }

    fn row(&self) -> RowKey {
        match self {
            PendingPull::Create(p) => (p.core.clone(), p.server.uuid.clone()),
            PendingPull::Sync(p) => (p.core.clone(), p.server.uuid.clone()),
        }
    }

    /// Removes the downloaded pack.
    fn discard(&self) {
        let path = match self {
            PendingPull::Create(p) => &p.pack_path,
            PendingPull::Sync(p) => &p.pack_path,
        };
        let _ = std::fs::remove_file(path);
    }
}

/// Asked before playing when the server's mods changed.
pub struct UpdatePrompt {
    pub core: String,
    pub server: ServerInstance,
    pub instance_id: String,
    pub changes: PackChanges,
    pending: Option<PendingSync>,
    pub focus: FocusHandle,
}

/// "This server's mods will run on your Mac": before installing code from a core that
/// isn't trusted for mods.
pub struct AdvisoryPrompt {
    pub core_name: String,
    pub server_name: String,
    pub advisory: Advisory,
    /// An update of an existing instance, rather than a new one.
    pub update: bool,
    pending: Option<PendingPull>,
    pub focus: FocusHandle,
}

/// What the user answers to the advisory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvisoryAnswer {
    InstallOnce,
    TrustAndInstall,
    Cancel,
}

/// A certificate macOS doesn't trust, shown with its fingerprint.
pub struct CertPrompt {
    pub address: String,
    pub host: String,
    /// The core's name: the saved one, or what the core says over the connection that
    /// isn't trusted yet.
    pub name: Option<String>,
    pub presented: String,
    /// The fingerprint trusted until now, when the certificate changed.
    pub previous: Option<String>,
    target: CertTarget,
    pub focus: FocusHandle,
}

#[derive(Clone)]
enum CertTarget {
    /// "Add server" continues once the certificate is trusted.
    Add(AddRequest, Entity<AddServerDialog>),
    /// A core on the page.
    Core(String),
}

/// "Do you trust <core>?": after adding a core, or turning "Trusted for mods" on.
pub struct TrustPrompt {
    pub core: String,
    pub name: String,
    pub address: String,
    /// Asked right after adding the core.
    pub after_add: bool,
    pub focus: FocusHandle,
}

pub enum ServersEvent {
    /// Show this launcher instance (it is starting).
    OpenInstance(String),
    /// Something failed while the page isn't shown (Play from the instance page).
    Failed {
        title: SharedString,
        message: SharedString,
    },
    /// Show the Servers page.
    ShowServers,
}

/// A row is keyed by core and server.
type RowKey = (String, String);

/// An instance linked to a server, for the instance page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceLink {
    pub server_name: String,
    pub core_name: String,
    /// The core is still in the launcher.
    pub core_known: bool,
}

/// How long the poll goes without reading the instances' server links again while
/// the instances stay the same.
const LINKS_REREAD: Duration = Duration::from_secs(60);

pub struct Servers {
    accounts: Entity<Accounts>,
    instances: Entity<Instances>,
    http: Arc<dyn HttpClient>,
    file: PathBuf,
    pub cores: Vec<CoreEntry>,
    /// Launcher instances made for a server.
    links: HashMap<RowKey, String>,
    /// The same links by instance.
    instance_links: HashMap<String, ServerLink>,
    /// Reads every instance folder for its link, off the window's thread.
    links_task: Option<Task<()>>,
    /// The instance list's generation the links were last read at, and when: the
    /// poll reads them again only once the instances changed, or a while after.
    links_read: Option<(u64, Instant)>,
    /// Goes up when a link is made or removed here, so a read that started before it
    /// is done again.
    links_revision: u64,
    pub busy: HashMap<RowKey, Busy>,
    /// A message at the top of the page.
    pub notice: Option<Notice>,
    dialog: DialogSlot<Servers, (Entity<AddServerDialog>, Subscription)>,
    pub prompt: DialogSlot<Servers, UpdatePrompt>,
    /// A server's page, shown instead of the list (see [`detail`]).
    detail: Option<(Entity<detail::ServerDetail>, Subscription)>,
    pub advisory: DialogSlot<Servers, AdvisoryPrompt>,
    pub cert_prompt: DialogSlot<Servers, CertPrompt>,
    pub trust_prompt: DialogSlot<Servers, TrustPrompt>,
    visible: bool,
    last_poll: Option<Instant>,
    /// No network, no Keychain: the offscreen screenshots.
    preview: bool,
    tasks: Vec<Task<()>>,
    _poller: Option<Task<()>>,
    /// Hosting and LAN worlds.
    extras: extras::Extras,
    /// "On this computer": cores found here, and what claiming them gave.
    pub local: local::LocalCores,
    /// Friends' servers (Lodestone Connect) and the join dialog.
    pub friends: friends::Friends,
    /// "Download Lodestone Host" is looking up the newest release.
    host_download: Option<Task<()>>,
    /// Closes a friend's tunnel when its game exits.
    _instances_changed: Option<Subscription>,
}

impl EventEmitter<ServersEvent> for Servers {}

impl Servers {
    pub fn new(
        accounts: Entity<Accounts>,
        instances: Entity<Instances>,
        cx: &mut Context<Self>,
    ) -> Self {
        let data_root = accounts.read(cx).data_dir().root().to_path_buf();
        let http = launcher_core::http::default_client().expect("could not start the HTTP client");
        let mut this = Self {
            accounts,
            instances,
            http,
            file: store::servers_file(&data_root),
            cores: Vec::new(),
            links: HashMap::new(),
            instance_links: HashMap::new(),
            links_task: None,
            links_read: None,
            links_revision: 0,
            busy: HashMap::new(),
            notice: None,
            dialog: DialogSlot::new(|this: &mut Servers| &mut this.dialog),
            prompt: DialogSlot::new(|this: &mut Servers| &mut this.prompt),
            detail: None,
            advisory: DialogSlot::new(|this: &mut Servers| &mut this.advisory),
            cert_prompt: DialogSlot::new(|this: &mut Servers| &mut this.cert_prompt),
            trust_prompt: DialogSlot::new(|this: &mut Servers| &mut this.trust_prompt),
            visible: false,
            last_poll: None,
            preview: false,
            tasks: Vec::new(),
            _poller: None,
            extras: Default::default(),
            local: Default::default(),
            friends: friends::Friends::new(&data_root),
            host_download: None,
            _instances_changed: None,
        };
        this._instances_changed =
            Some(cx.observe(&this.instances, |this, _, cx| this.friend_games_changed(cx)));
        this.load(cx);
        this.load_friends();
        this.reload_links(cx);
        this
    }

    /// Reads `servers.json` and the keys.
    fn load(&mut self, cx: &mut Context<Self>) {
        let mut file = match store::load(&self.file) {
            Ok(file) => file,
            Err(err) => {
                let err = err.to_string();
                self.notice = Some(Notice::new(move || (t(M::ReadFailed).into(), err.clone())));
                ServersFile::default()
            }
        };
        // signing in used to add a second entry for a core added anonymously
        let merged = store::dedupe(&mut file.cores, |c| c, None);
        self.cores = file
            .cores
            .into_iter()
            .filter_map(|saved| {
                let url = cores::parse_address(&saved.url).ok()?;
                Some(CoreEntry::new(saved, url, None))
            })
            .collect();
        if !merged.is_empty() {
            self.merged(merged, cx);
            self.save();
        }
        // cores claimed on this computer keep their key in plain text, not the Keychain
        self.load_local_credentials(cx);
        let secrets = self.accounts.read(cx).secrets().clone();
        let signed_in: Vec<String> = self
            .cores
            .iter()
            .filter(|c| c.saved.signed_in() && !self.has_local_key(&c.saved.uuid))
            .map(|c| c.saved.uuid.clone())
            .collect();
        if signed_in.is_empty() {
            return;
        }
        self.tasks.push(cx.spawn(async move |this, cx| {
            let store = secrets.store(cx);
            let mut keys = Vec::new();
            for uuid in signed_in {
                if let Ok(Some(key)) = store::read_key(&*store, &uuid).await {
                    keys.push((uuid, key));
                }
            }
            this.update(cx, |this, cx| {
                for (uuid, key) in keys {
                    if let Some(core) = this.cores.iter_mut().find(|c| c.saved.uuid == uuid) {
                        core.key = Some(key);
                    }
                }
                if this.visible {
                    this.refresh_all(cx);
                }
            })
            .ok();
        }));
    }

    fn save(&mut self) {
        if self.preview {
            return;
        }
        let file = ServersFile {
            cores: self.cores.iter().map(|c| c.saved.clone()).collect(),
        };
        if let Err(err) = store::save(&self.file, &file) {
            let err = err.to_string();
            self.notice = Some(Notice::new(move || (t(M::SaveFailed).into(), err.clone())));
        }
    }

    /// The page is shown or hidden; lists are polled only while it is shown.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.lan_visible(visible, cx);
        if visible {
            self.scan_local(false, cx);
        }
        if !visible || self.preview {
            self._poller = None;
            return;
        }
        if self
            .last_poll
            .is_none_or(|at| at.elapsed() > Duration::from_secs(1))
        {
            self.refresh_all(cx);
        }
        self._poller = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.visible && this.detail.is_none() {
                            this.refresh_all(cx);
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        self.last_poll = Some(Instant::now());
        self.reload_links_if_stale(cx);
        for index in 0..self.cores.len() {
            self.refresh(index, cx);
        }
    }

    /// Finds the launcher instances made for a server, off the window's thread: it
    /// reads every instance folder.
    fn reload_links(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let root = self.accounts.read(cx).data_dir().instances_dir();
        let instances = self.instances.read(cx);
        let existing: HashSet<String> = instances.list().iter().map(|i| i.id.clone()).collect();
        self.links_read = Some((instances.list_generation(), Instant::now()));
        let revision = self.links_revision;
        let read = cx.background_spawn(async move {
            link::linked_instances(&root)
                .into_iter()
                .filter(|(id, _)| existing.contains(id))
                .collect::<Vec<(String, ServerLink)>>()
        });
        self.links_task = Some(cx.spawn(async move |this, cx| {
            let linked = read.await;
            this.update(cx, |this, cx| {
                this.links_task = None;
                if this.links_revision != revision {
                    // a link was made or removed meanwhile: what was read may miss it
                    this.reload_links(cx);
                    return;
                }
                this.links = linked
                    .iter()
                    .map(|(id, link)| {
                        (
                            (link.core_uuid.clone(), link.server_uuid.clone()),
                            id.clone(),
                        )
                    })
                    .collect();
                this.instance_links = linked.into_iter().collect();
                cx.notify();
            })
            .ok();
        }));
    }

    /// The poll's look at the links: again only once the instances changed, or after
    /// [`LINKS_REREAD`] in case a link file changed outside the launcher.
    fn reload_links_if_stale(&mut self, cx: &mut Context<Self>) {
        let generation = self.instances.read(cx).list_generation();
        if let Some((read, at)) = self.links_read
            && read == generation
            && at.elapsed() < LINKS_REREAD
        {
            return;
        }
        self.reload_links(cx);
    }

    fn client(&self, core: &CoreEntry) -> CoreClient {
        let client = CoreClient::new(self.http.clone(), core.url.clone());
        let client = match &core.pinned {
            Some(pinned) => client.with_pinned(pinned.clone()),
            None => client,
        };
        // a claimed core's owner key goes only to its certificate
        if core.saved.local_claim {
            client.strict()
        } else {
            client
        }
    }

    /// The client (and key) to pull a server's client pack with: the core's, or a
    /// friend's tunnel's info port while it is open.
    fn pull_client(&self, core_uuid: &str) -> Option<(CoreClient, Option<Secret>)> {
        match self.core_index(core_uuid) {
            Some(index) => {
                let core = &self.cores[index];
                Some((self.client(core), core.key.clone()))
            }
            None => Some((self.friend_client(core_uuid)?, None)),
        }
    }

    /// The key an entry keeps when its core is added again at `address` without signing
    /// in: only a key for this very address, and never a claimed one (it is tied to
    /// the claim's address and certificate).
    fn carried_key(&self, existing: Option<usize>, address: &str) -> Option<Secret> {
        let core = self.cores.get(existing?)?;
        if display_address(&core.url) != address || core.saved.local_claim {
            return None;
        }
        core.key.clone()
    }

    fn refresh(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(core) = self.cores.get(index) else {
            return;
        };
        if core.refreshing {
            return;
        }
        let client = self.client(core);
        let key = core.key.clone();
        let uuid = core.saved.uuid.clone();
        self.cores[index].refreshing = true;
        let fetch = cx.background_spawn(async move { fetch_list(&client, key.as_ref()).await });
        self.tasks.retain(|t| !t.is_ready());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                let Some(index) = this.core_index(&uuid) else {
                    return;
                };
                this.cores[index].refreshing = false;
                match result {
                    Ok(list) => {
                        this.listed(index, list, cx);
                    }
                    Err(CoreError::SignedOut) => this.signed_out(index, cx),
                    Err(err) => this.cores[index].status = CoreStatus::Failed(err),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// A list arrived: keep the name, and when another id answers, end trust and move
    /// the card (with its key and linked instances) to that id. Returns the core's id.
    fn listed(&mut self, index: usize, list: ServerList, cx: &mut Context<Self>) -> String {
        let core = &mut self.cores[index];
        let mut changed = false;
        if core.saved.name != list.core_name {
            core.saved.name = list.core_name.clone();
            changed = true;
        }
        let renamed = core.saved.adopt_identity(&list.core_uuid);
        let uuid = core.saved.uuid.clone();
        if let Some((_, trust_ended)) = &renamed {
            changed = true;
            if *trust_ended {
                let (name, address) = (core.saved.name.clone(), display_address(&core.url));
                self.notice = Some(Notice::new(move || {
                    (
                        tr!(M::NoLongerTrusted, name = name),
                        tr!(M::NoLongerTrustedIdChanged, address = address),
                    )
                }));
            }
        }
        self.cores[index].status = CoreStatus::Online(list);
        if let Some((old, _)) = renamed {
            // the key this device had is saved under the old id
            if self.cores[index].saved.local_claim {
                self.rename_local_credentials(&old.uuid, &uuid, cx);
            } else if let Some(key) = self.cores[index].key.clone() {
                self.save_key(uuid.clone(), key, cx);
            }
            let mut merged = store::dedupe(&mut self.cores, |c| &c.saved, Some(index));
            merged.insert(
                0,
                store::Merged {
                    removed: old,
                    into: uuid.clone(),
                },
            );
            self.merged(merged, cx);
        }
        if changed {
            self.save();
        }
        uuid
    }

    /// The key stopped working: forget it, say so, and show the public list.
    fn signed_out(&mut self, index: usize, cx: &mut Context<Self>) {
        let core = &mut self.cores[index];
        core.key = None;
        core.saved.username = None;
        core.saved.key_id = None;
        core.saved.key_expires_at = None;
        let uuid = core.saved.uuid.clone();
        let name = core.saved.name.clone();
        self.save();
        self.notice = Some(Notice::new(move || {
            (
                tr!(M::SignedOutOf, name = name),
                core_error(&CoreError::SignedOut),
            )
        }));
        self.forget_key(uuid, cx);
        self.refresh(index, cx);
    }

    /// Entries were merged into others for the same core (see [`store::dedupe`]): point
    /// their instances at the entry that stays, and forget keys saved under ids no entry
    /// has any more.
    fn merged(&mut self, merged: Vec<store::Merged>, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let root = self.accounts.read(cx).data_dir().instances_dir();
        for store::Merged { removed, into } in &merged {
            if let Some(index) = self.core_index(into) {
                let url = self.cores[index].saved.url.clone();
                link::relink(&root, &removed.uuid, into, &url);
            }
        }
        let gone: HashSet<String> = merged
            .into_iter()
            .map(|m| m.removed.uuid)
            .filter(|uuid| self.core_index(uuid).is_none())
            .collect();
        for uuid in gone {
            self.forget_key(uuid, cx);
        }
        self.reload_links(cx);
    }

    fn save_key(&mut self, uuid: String, key: Secret, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let secrets = self.accounts.read(cx).secrets().clone();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let store = secrets.store(cx);
            if let Err(err) = store::write_key(&*store, &uuid, &key).await {
                this.update(cx, |this, cx| {
                    let err = format!("{err:#}");
                    this.notice = Some(Notice::new(move || {
                        (
                            tr!(M::KeySaveFailed, keychain = t(CommonMsg::Keychain)),
                            err.clone(),
                        )
                    }));
                    cx.notify();
                })
                .ok();
            }
        }));
    }

    fn forget_key(&mut self, uuid: String, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let secrets = self.accounts.read(cx).secrets().clone();
        self.tasks.push(cx.spawn(async move |_, cx| {
            let store = secrets.store(cx);
            let _ = store::delete_key(&*store, &uuid).await;
        }));
    }

    pub fn dialog(&self) -> Option<&Entity<AddServerDialog>> {
        self.dialog.current().map(|(dialog, _)| dialog)
    }

    /// Closes "Add server" if `dialog` is the open one: one that is fading out has
    /// nothing more to say.
    fn close_add_dialog(&mut self, dialog: &Entity<AddServerDialog>, cx: &mut Context<Self>) {
        if self.dialog() == Some(dialog) {
            self.dialog.close(cx);
        }
    }

    /// "Add server", or "Sign in" for a core that is already added (`existing`).
    pub fn open_add(&mut self, existing: Option<usize>, cx: &mut Context<Self>) {
        let prefill = existing
            .and_then(|i| self.cores.get(i))
            .map(|c| display_address(&c.url));
        let dialog = cx.new(|cx| AddServerDialog::new(prefill, cx));
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| match event {
            AddServerEvent::Dismissed => this.close_add_dialog(&dialog, cx),
            AddServerEvent::Submit(request) if this.dialog() == Some(&dialog) => {
                this.add_core(request.clone(), dialog, None, cx)
            }
            AddServerEvent::Submit(_) => {}
        });
        self.dialog.open((dialog, subscription), cx);
    }

    /// Adds (or signs in to) the core at `request.address`. `accepted` is a certificate
    /// the user just trusted for it.
    fn add_core(
        &mut self,
        request: AddRequest,
        dialog: Entity<AddServerDialog>,
        accepted: Option<CertificatePin>,
        cx: &mut Context<Self>,
    ) {
        let url = match cores::parse_address(&request.address) {
            Ok(url) => url,
            Err(err) => {
                dialog.update(cx, |d, cx| d.fail(core_error(&err), cx));
                return;
            }
        };
        let address = display_address(&url);
        let pin = accepted.clone().or_else(|| {
            self.cores
                .iter()
                .find(|c| display_address(&c.url) == address)
                .and_then(|c| c.saved.certificate.clone())
        });
        let pinned = pinned_http(pin.as_ref());
        let client = match &pinned {
            Some(pinned) => {
                CoreClient::new(self.http.clone(), url.clone()).with_pinned(pinned.clone())
            }
            None => CoreClient::new(self.http.clone(), url.clone()),
        };
        let http = self.http.clone();
        let secrets = self.accounts.read(cx).secrets().clone();
        let device = store::device_key_name(&store::host_name());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let credentials = request.username.clone().zip(request.password.clone());
            let expires_at = request.expires_at;
            let console = request.console;
            let probe_url = url.clone();
            let result = cx
                .background_spawn(async move {
                    let info = match client.info().await {
                        Ok(info) => info,
                        Err(err) => return Err(certificate_outcome(err, &http, probe_url).await),
                    };
                    // an old core has no list: fail now rather than after adding it
                    client.public_servers().await.map_err(AddFailure::Core)?;
                    let key = match credentials {
                        Some((username, password)) => Some(
                            client
                                .sign_in_with_scope(
                                    &username, &password, &device, expires_at, console,
                                )
                                .await
                                .map_err(AddFailure::Core)?,
                        ),
                        None => None,
                    };
                    Ok::<_, AddFailure>((info, key))
                })
                .await;
            let (info, key) = match result {
                Ok(done) => done,
                Err(AddFailure::Certificate {
                    host,
                    presented,
                    previous,
                    name,
                }) => {
                    this.update(cx, |this, cx| {
                        dialog.update(cx, |d, cx| d.fail(t(M::CheckCertificate).into(), cx));
                        let focus = cx.focus_handle();
                        this.cert_prompt.open(
                            CertPrompt {
                                address: address.clone(),
                                host,
                                name,
                                presented,
                                previous,
                                target: CertTarget::Add(request, dialog.clone()),
                                focus,
                            },
                            cx,
                        );
                    })
                    .ok();
                    return;
                }
                Err(AddFailure::Core(err)) => {
                    dialog.update(cx, |d, cx| d.fail(core_error(&err), cx));
                    return;
                }
            };
            if let Some((secret, _)) = &key {
                let store = secrets.store(cx);
                if let Err(err) = store::write_key(&*store, &info.uuid, secret).await {
                    dialog.update(cx, |d, cx| {
                        d.fail(
                            tr!(
                                M::KeySaveFailedError,
                                keychain = t(CommonMsg::Keychain),
                                error = format!("{err:#}")
                            ),
                            cx,
                        )
                    });
                    return;
                }
            }
            this.update(cx, |this, cx| {
                // by id, else by address: a core makes a new id each time it starts, and
                // signing in from its card must update that card, not add another
                let saved_cores: Vec<SavedCore> =
                    this.cores.iter().map(|c| c.saved.clone()).collect();
                let existing = store::find_existing(&saved_cores, &info.uuid, &address);
                // another core used to answer at this address
                for core in &mut this.cores {
                    if display_address(&core.url) == address
                        && core.saved.uuid != info.uuid
                        && core.saved.check_identity(&info.uuid)
                    {
                        let (name, address) = (core.saved.name.clone(), address.clone());
                        this.notice = Some(Notice::new(move || {
                            (
                                tr!(M::NoLongerTrusted, name = name),
                                tr!(M::NoLongerTrustedOtherCore, address = address),
                            )
                        }));
                    }
                }
                let mut saved = match existing {
                    Some(index) => this.cores[index].saved.clone(),
                    None => SavedCore::default(),
                };
                // the entry's id before this, if it changes
                let renamed = existing
                    .map(|index| this.cores[index].saved.clone())
                    .filter(|old| old.uuid != info.uuid);
                let was_trusted = saved.is_trusted();
                saved.uuid = info.uuid.clone();
                saved.url = address.clone();
                saved.name = info.core_name.clone();
                saved.product = info.product;
                saved.launched_by = info.launched_by;
                let kept_key = this.carried_key(existing, &address);
                if key.is_some() {
                    saved.username = request.username.clone();
                    saved.key_id = key.as_ref().map(|(_, info)| info.id.clone());
                    saved.key_expires_at = key.as_ref().and_then(|(_, info)| info.expires_at);
                    saved.local_claim = false;
                } else if kept_key.is_none() {
                    // no key goes with the entry to another address
                    saved.username = None;
                    saved.key_id = None;
                    saved.key_expires_at = None;
                    saved.local_claim = false;
                }
                if let Some(pin) = pin.clone()
                    && saved.pin_certificate(pin)
                {
                    let name = saved.name.clone();
                    this.notice = Some(Notice::new(move || {
                        (
                            tr!(M::NoLongerTrusted, name = name),
                            t(M::NoLongerTrustedNewCertificate).into(),
                        )
                    }));
                }
                let signed_in_now = key.is_some();
                let entry = CoreEntry::new(
                    saved,
                    url,
                    key.map(|(secret, _)| secret).or(kept_key.clone()),
                );
                let trusted = entry.saved.is_trusted();
                let name = entry.saved.name.clone();
                let uuid = entry.saved.uuid.clone();
                let index = match existing {
                    Some(index) => {
                        this.cores[index] = entry;
                        index
                    }
                    None => {
                        this.cores.push(entry);
                        this.cores.len() - 1
                    }
                };
                // other entries for this core (from before duplicates were merged) go
                let mut merged = store::dedupe(&mut this.cores, |c| &c.saved, Some(index));
                if let Some(old) = renamed {
                    // the key this device had is saved under the old id
                    if !signed_in_now && let Some(key) = kept_key {
                        this.save_key(uuid.clone(), key, cx);
                    }
                    merged.insert(
                        0,
                        store::Merged {
                            removed: old,
                            into: uuid.clone(),
                        },
                    );
                }
                this.merged(merged, cx);
                let index = this.core_index(&uuid).unwrap_or(index);
                this.save();
                this.close_add_dialog(&dialog, cx);
                if existing.is_none() && !trusted && !was_trusted {
                    let focus = cx.focus_handle();
                    this.trust_prompt.open(
                        TrustPrompt {
                            core: uuid,
                            name,
                            address: address.clone(),
                            after_add: true,
                            focus,
                        },
                        cx,
                    );
                }
                this.refresh(index, cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// "Trust certificate…" on a core whose certificate macOS doesn't trust, or "Trust
    /// the new certificate…" after it changed.
    pub fn review_certificate(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(core) = self.cores.get(index) else {
            return;
        };
        let (host, presented, previous) = match &core.status {
            CoreStatus::Failed(CoreError::Untrusted {
                host,
                fingerprint: Some(fingerprint),
            }) => (host.clone(), fingerprint.clone(), None),
            CoreStatus::Failed(CoreError::CertificateChanged {
                host,
                pinned,
                presented,
            }) => (host.clone(), presented.clone(), Some(pinned.clone())),
            _ => return,
        };
        let prompt = CertPrompt {
            address: display_address(&core.url),
            host,
            name: Some(core.saved.name.clone()),
            presented,
            previous,
            target: CertTarget::Core(core.saved.uuid.clone()),
            focus: cx.focus_handle(),
        };
        self.cert_prompt.open(prompt, cx);
    }

    /// The answer to the certificate prompt.
    pub fn answer_certificate(&mut self, trust: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.cert_prompt.close(cx) else {
            return;
        };
        let pin = CertificatePin::new(&prompt.host, &prompt.presented);
        match prompt.target.clone() {
            CertTarget::Add(request, dialog) => {
                if trust {
                    dialog.update(cx, |d, cx| d.set_busy(cx));
                    self.add_core(request, dialog, Some(pin), cx);
                } else {
                    dialog.update(cx, |d, cx| d.fail(t(M::CertificateNotTrusted).into(), cx));
                }
            }
            CertTarget::Core(uuid) if trust => {
                let Some(index) = self.core_index(&uuid) else {
                    return;
                };
                let core = &mut self.cores[index];
                if core.saved.pin_certificate(pin) {
                    let name = core.saved.name.clone();
                    self.notice = Some(Notice::new(move || {
                        (
                            tr!(M::NoLongerTrusted, name = name),
                            t(M::NoLongerTrustedCertificateChanged).into(),
                        )
                    }));
                }
                core.pinned = pinned_http(core.saved.certificate.as_ref());
                core.status = CoreStatus::Loading;
                self.save();
                self.refresh(index, cx);
            }
            CertTarget::Core(_) => {}
        }
        cx.notify();
    }

    /// "Trusted for mods" on a core's card: turning it on asks first.
    pub fn set_trusted(&mut self, index: usize, trusted: bool, cx: &mut Context<Self>) {
        let Some(core) = self.cores.get_mut(index) else {
            return;
        };
        if trusted {
            let prompt = TrustPrompt {
                core: core.saved.uuid.clone(),
                name: core.saved.name.clone(),
                address: display_address(&core.url),
                after_add: false,
                focus: cx.focus_handle(),
            };
            self.trust_prompt.open(prompt, cx);
        } else {
            core.saved.set_trusted(false);
            self.save();
        }
        cx.notify();
    }

    /// The answer to "Do you trust <core>?".
    pub fn answer_trust(&mut self, trust: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.trust_prompt.close(cx) else {
            return;
        };
        let core = prompt.core.clone();
        if trust && let Some(index) = self.core_index(&core) {
            self.cores[index].saved.set_trusted(true);
            self.save();
        }
        cx.notify();
    }

    /// Forgets the core's key on this device. The key stays valid on the core until it
    /// is revoked in the dashboard, which the notice says.
    pub fn sign_out(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(uuid) = self.cores.get(index).map(|c| c.saved.uuid.clone()) else {
            return;
        };
        // a key claimed on this computer is revoked on the core first, best effort
        self.revoke_local_key(&uuid, cx);
        let Some(core) = self.cores.get_mut(index) else {
            return;
        };
        core.saved.local_claim = false;
        let name = core.saved.name.clone();
        let key_id = core.saved.key_id.take();
        core.key = None;
        core.saved.username = None;
        core.saved.key_expires_at = None;
        let uuid = core.saved.uuid.clone();
        self.save();
        self.forget_local_key(&uuid, cx);
        self.forget_key(uuid, cx);
        if let Some(id) = key_id {
            self.notice = Some(Notice::info(move || {
                (
                    tr!(M::SignedOutOf, name = name),
                    tr!(M::KeyStillValid, id = id),
                )
            }));
        }
        self.refresh(index, cx);
        cx.notify();
    }

    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.cores.len() {
            return;
        }
        let uuid = self.cores[index].saved.uuid.clone();
        self.revoke_local_key(&uuid, cx);
        let core = self.cores.remove(index);
        self.save();
        if core.signed_in() {
            self.forget_local_key(&core.saved.uuid, cx);
            self.forget_key(core.saved.uuid, cx);
        }
        cx.notify();
    }

    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }

    /// Says that something failed: at the top of the page, and to the shell when the
    /// page isn't shown. `text` makes the title and the message.
    fn fail(&mut self, text: impl Fn() -> (String, String) + 'static, cx: &mut Context<Self>) {
        let (title, message) = text();
        if self.friend_failure(&title, &message, cx) {
            return;
        }
        let notice = Notice::new(text);
        if !self.visible {
            let (title, message) = notice.text();
            cx.emit(ServersEvent::Failed {
                title: title.into(),
                message: message.into(),
            });
        }
        self.notice = Some(notice);
        cx.notify();
    }

    /// A failure of a request to a core: the key stopped working, or something else.
    /// `title` makes the notice's title.
    fn fail_core(
        &mut self,
        core: &str,
        title: impl Fn() -> String + 'static,
        err: impl Into<PullError>,
        cx: &mut Context<Self>,
    ) {
        match err.into() {
            PullError::Core(CoreError::SignedOut) => {
                if let Some(index) = self.core_index(core) {
                    self.signed_out(index, cx);
                }
            }
            PullError::Cancelled => {}
            err => self.fail(move || (title(), err.to_string()), cx),
        }
    }

    pub fn linked_instance(&self, core: &str, server: &str) -> Option<&String> {
        self.links.get(&(core.to_string(), server.to_string()))
    }

    /// The server an instance is linked to, for the instance page.
    pub fn instance_link(&self, instance_id: &str) -> Option<InstanceLink> {
        let link = self.instance_links.get(instance_id)?;
        if cores::friends::is_friend_id(&link.core_uuid) {
            let friend = self.friends.get(&link.core_uuid);
            return Some(InstanceLink {
                server_name: link.server_name.clone(),
                core_name: friend
                    .map_or_else(|| link.core_url.clone(), |f| f.saved.core_name.clone()),
                core_known: friend.is_some(),
            });
        }
        let core = self.cores.iter().find(|c| c.saved.uuid == link.core_uuid);
        Some(InstanceLink {
            server_name: link.server_name.clone(),
            core_name: core.map_or_else(|| link.core_url.clone(), |c| c.saved.name.clone()),
            core_known: core.is_some(),
        })
    }

    /// Forgets which server the instance was made for, so Play no longer joins it or
    /// keeps its mods in step. The instance keeps its mods.
    pub fn unlink_instance(
        &mut self,
        instance_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let dir = self.instances.read(cx).instance_dir(instance_id);
        let path = link::link_path(&dir);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|err| tr!(M::UnlinkFailed, error = err))?;
        }
        self.instance_links.remove(instance_id);
        self.links.retain(|_, id| id != instance_id);
        self.links_revision += 1;
        cx.notify();
        Ok(())
    }

    /// What the instance's server row is doing, for the instance page.
    pub fn instance_busy(&self, instance_id: &str) -> Option<&Busy> {
        let link = self.instance_links.get(instance_id)?;
        self.busy
            .get(&(link.core_uuid.clone(), link.server_uuid.clone()))
    }

    fn core_index(&self, uuid: &str) -> Option<usize> {
        self.cores.iter().position(|c| c.saved.uuid == uuid)
    }

    /// Redraws while `row` is busy, for its progress.
    fn tick_while_busy(&mut self, row: RowKey, cx: &mut Context<Self>) {
        self.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let busy = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        this.busy.contains_key(&row)
                    })
                    .unwrap_or(false);
                if !busy {
                    break;
                }
            }
        }));
    }

    /// "Create matching instance": a launcher instance with the server's Minecraft
    /// version and loader. A server with a client pack gets it through the modpack
    /// import, after the advisory when the core isn't trusted for mods.
    pub fn create_instance(
        &mut self,
        core_uuid: &str,
        server: ServerInstance,
        cx: &mut Context<Self>,
    ) {
        let Some((client, key)) = self.pull_client(core_uuid) else {
            return;
        };
        if !server.has_pack() {
            return self.create_plain(core_uuid, &server, cx);
        }
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        self.busy.insert(row.clone(), Busy::Fetching);
        let dir = self.instances.read(cx).store().root().to_path_buf();
        let fetch_server = server.clone();
        let work = cx.background_spawn(async move {
            let (bytes, etag) = match client
                .client_pack(key.as_ref(), &fetch_server, None)
                .await?
            {
                PackFetch::Pack { bytes, etag } => (bytes, etag),
                PackFetch::NoMods(_) | PackFetch::NotModified => return Ok(None),
            };
            let pack_path = sync::write_pack(&dir, &bytes)?;
            let read = modpack::read_pack(&pack_path)
                .map_err(PullError::from)
                .and_then(|pack| {
                    let loader =
                        sync::loader_for(Some(&pack), &fetch_server).map_err(PullError::Other)?;
                    Ok((pack, loader))
                });
            match read {
                Ok((pack, loader)) => Ok(Some((pack_path, pack, etag, loader))),
                Err(err) => {
                    let _ = std::fs::remove_file(&pack_path);
                    Err(err)
                }
            }
        });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(None) => this.create_plain(&core_uuid, &server, cx),
                    Ok(Some((pack_path, pack, etag, loader))) => {
                        let pending = PendingPull::Create(PendingCreate {
                            core: core_uuid.clone(),
                            server,
                            pack_path,
                            pack: Arc::new(pack),
                            etag,
                            loader,
                        });
                        this.gate(pending, PullKind::ClientPack, cx);
                    }
                    Err(err) => this.fail_core(&core_uuid, create_failed, err, cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// A matching instance without the server's pack (it needs no client mods).
    fn create_plain(&mut self, core_uuid: &str, server: &ServerInstance, cx: &mut Context<Self>) {
        let loader = match sync::loader_for(None, server) {
            Ok(loader) => loader,
            Err(message) => {
                return self.fail(move || (create_failed(), message.clone()), cx);
            }
        };
        let created = self.instances.update(cx, |instances, cx| {
            instances.create(&server.name, &server.game_version, loader.as_ref(), cx)
        });
        match created {
            Ok(instance) => {
                self.link_instance(core_uuid, server, &instance.id, None, cx);
                if server.mods_hidden() {
                    let name = server.name.clone();
                    self.notice = Some(Notice::new(move || {
                        (
                            tr!(M::CreatedWithoutMods, name = name),
                            t(M::CreatedWithoutModsBody).into(),
                        )
                    }));
                }
            }
            Err(message) => self.fail(move || (create_failed(), message.clone()), cx),
        }
    }

    /// Records that `instance_id` was made for `server`.
    fn link_instance(
        &mut self,
        core_uuid: &str,
        server: &ServerInstance,
        instance_id: &str,
        etag: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let core_url = match self.core_index(core_uuid) {
            Some(index) => display_address(&self.cores[index].url),
            None => match self.friends.get(core_uuid) {
                Some(friend) => friend.saved.link.clone(),
                None => return,
            },
        };
        let link = ServerLink {
            core_uuid: core_uuid.to_string(),
            core_url,
            server_uuid: server.uuid.clone(),
            server_name: server.name.clone(),
            pack_etag: etag,
        };
        let instance_dir = self.instances.read(cx).content_dirs(instance_id).instance;
        if let Err(err) = link::save_link(&instance_dir, &link) {
            let err = err.to_string();
            return self.fail(move || (t(M::LinkFailed).into(), err.clone()), cx);
        }
        self.links.insert(
            (link.core_uuid.clone(), link.server_uuid.clone()),
            instance_id.to_string(),
        );
        self.instance_links.insert(instance_id.to_string(), link);
        self.links_revision += 1;
        self.friend_linked(core_uuid, instance_id, cx);
        cx.notify();
    }

    /// Installs `pending` from its core, after the advisory unless the user trusts the
    /// core for mods.
    fn gate(&mut self, pending: PendingPull, kind: PullKind, cx: &mut Context<Self>) {
        let source = match self.core_index(pending.core()) {
            Some(index) => Some((
                trust::needs_advisory(&self.cores[index].saved, kind),
                self.cores[index].saved.name.clone(),
            )),
            None => self.friend_pull_source(pending.core(), kind),
        };
        let Some((needs_advisory, core_name)) = source else {
            pending.discard();
            return;
        };
        if !needs_advisory {
            return self.install(pending, cx);
        }
        let files: Vec<PulledFile> = match &pending {
            PendingPull::Create(p) => sync::pack_pulls(&p.pack),
            PendingPull::Sync(p) => sync::plan_pulls(&p.plan),
        };
        let server_name = match &pending {
            PendingPull::Create(p) => p.server.name.clone(),
            PendingPull::Sync(p) => p.server.name.clone(),
        };
        let row = pending.row();
        self.busy.insert(row.clone(), Busy::CheckingMods);
        let api = self.instances.read(cx).modrinth();
        let work =
            cx.background_spawn(async move { trust::advisory(api.as_deref(), &files).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let advisory = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                let update = matches!(pending, PendingPull::Sync(_));
                let focus = cx.focus_handle();
                this.advisory.open(
                    AdvisoryPrompt {
                        core_name,
                        server_name,
                        advisory,
                        update,
                        pending: Some(pending),
                        focus,
                    },
                    cx,
                );
            })
            .ok();
        }));
        cx.notify();
    }

    /// The answer to the advisory.
    pub fn answer_advisory(&mut self, answer: AdvisoryAnswer, cx: &mut Context<Self>) {
        let Some(prompt) = self.advisory.close(cx) else {
            return;
        };
        let Some(pending) = prompt.pending.take() else {
            cx.notify();
            return;
        };
        match answer {
            AdvisoryAnswer::Cancel => {
                pending.discard();
                let core = pending.core().to_string();
                self.friend_cancelled(&core, cx);
            }
            AdvisoryAnswer::InstallOnce => self.install(pending, cx),
            AdvisoryAnswer::TrustAndInstall => {
                if let Some(index) = self.core_index(pending.core()) {
                    self.cores[index].saved.set_trusted(true);
                    self.save();
                } else {
                    let core = pending.core().to_string();
                    self.trust_friend(&core);
                }
                self.install(pending, cx);
            }
        }
        cx.notify();
    }

    fn install(&mut self, pending: PendingPull, cx: &mut Context<Self>) {
        match pending {
            PendingPull::Create(pending) => self.import_pack(pending, cx),
            PendingPull::Sync(pending) => self.apply_sync(pending, cx),
        }
    }

    /// Makes the instance from the server's pack, through the modpack import.
    fn import_pack(&mut self, pending: PendingCreate, cx: &mut Context<Self>) {
        let row: RowKey = (pending.core.clone(), pending.server.uuid.clone());
        let instances = self.instances.read(cx);
        let store = instances.store();
        let api = instances.modrinth();
        let downloader = instances.content_downloader();
        let progress = Arc::new(InstallProgress::default());
        self.busy
            .insert(row.clone(), Busy::Creating(progress.clone()));
        self.tick_while_busy(row.clone(), cx);
        let name = pending.server.name.clone();
        let pack = pending.pack.clone();
        let pack_path = pending.pack_path.clone();
        let loader = pending.loader.clone();
        let work = cx.background_spawn(async move {
            let result = import::import(
                &store,
                api.as_deref(),
                &downloader,
                &pack_path,
                &pack,
                ImportOptions {
                    name,
                    loader,
                    record: sync::record(&pack),
                    icon: None,
                },
                &progress,
                &CancelToken::new(),
            )
            .await;
            let _ = std::fs::remove_file(&pack_path);
            result
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(instance) => {
                        this.instances.update(cx, |instances, cx| {
                            instances.reload(cx);
                            instances.select(&instance.id, cx);
                        });
                        let skipped = pending.pack.skipped.len();
                        let name = instance.name.clone();
                        let version = instance.game_version.clone();
                        let loader = instance.loader_label();
                        this.notice = Some(Notice::success(move || {
                            let body = match skipped {
                                0 => tr!(M::ReadyBody, version = version, loader = loader),
                                n => {
                                    trn!(P::ReadyBodySkipped, n, version = version, loader = loader)
                                }
                            };
                            (tr!(M::ReadyTitle, name = name), body)
                        }));
                        // after the notice: a friend's server's Play goes on from here
                        this.link_instance(
                            &pending.core,
                            &pending.server,
                            &instance.id,
                            pending.etag.clone(),
                            cx,
                        );
                    }
                    Err(err) => this.fail_core(&pending.core, create_failed, err, cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Play: checks that the instance's mods match the server's, then joins.
    pub fn play(&mut self, core_uuid: &str, server: ServerInstance, cx: &mut Context<Self>) {
        let Some((client, key)) = self.pull_client(core_uuid) else {
            return;
        };
        let Some(instance_id) = self.linked_instance(core_uuid, &server.uuid).cloned() else {
            return;
        };
        if self.instances.read(cx).is_active(&instance_id) {
            cx.emit(ServersEvent::OpenInstance(instance_id));
            return;
        }
        if !server.has_pack() {
            return self.join(core_uuid, &server, &instance_id, cx);
        }
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        if self.busy.contains_key(&row) {
            return;
        }
        self.busy.insert(row.clone(), Busy::CheckingMods);
        let dirs = self.instances.read(cx).content_dirs(&instance_id);
        let check_server = server.clone();
        let check = cx.background_spawn(async move {
            let link = link::load_link(&dirs.instance).unwrap_or_default();
            let mut fetched = client
                .client_pack(key.as_ref(), &check_server, link.pack_etag.as_deref())
                .await?;
            if matches!(fetched, PackFetch::NotModified) {
                if sync::mods_on_disk(&dirs) {
                    return Ok(Check::InStep);
                }
                // files changed on disk: get the whole pack to compare
                fetched = client
                    .client_pack(key.as_ref(), &check_server, None)
                    .await?;
            }
            let (bytes, etag) = match fetched {
                PackFetch::Pack { bytes, etag } => (bytes, etag),
                PackFetch::NoMods(_) | PackFetch::NotModified => return Ok(Check::InStep),
            };
            let pack_path = sync::write_pack(&dirs.instance, &bytes)?;
            let planned = modpack::read_pack(&pack_path)
                .map_err(PullError::from)
                .and_then(|pack| {
                    let loader =
                        sync::loader_for(Some(&pack), &check_server).map_err(PullError::Other)?;
                    Ok(sync::plan(&dirs, &pack, loader))
                });
            let plan = match planned {
                Ok(plan) => plan,
                Err(err) => {
                    let _ = std::fs::remove_file(&pack_path);
                    return Err(err);
                }
            };
            if !plan.changes_files() {
                // the same mods in a rebuilt pack: remember it
                let _ = std::fs::remove_file(&pack_path);
                let _ = plan.record.save(&dirs.instance);
                let mut link = link;
                link.pack_etag = etag;
                let _ = link::save_link(&dirs.instance, &link);
                return Ok(Check::InStep);
            }
            Ok(Check::Changed(Box::new((pack_path, plan, etag))))
        });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = check.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(Check::InStep) => this.join(&core_uuid, &server, &instance_id, cx),
                    Ok(Check::Changed(boxed)) => {
                        let (pack_path, plan, etag) = *boxed;
                        let changes = PackChanges::from_plan(&plan);
                        let focus = cx.focus_handle();
                        this.prompt.open(
                            UpdatePrompt {
                                core: core_uuid.clone(),
                                server: server.clone(),
                                instance_id: instance_id.clone(),
                                changes,
                                pending: Some(PendingSync {
                                    core: core_uuid.clone(),
                                    server,
                                    instance_id,
                                    pack_path,
                                    plan: Arc::new(plan),
                                    etag,
                                }),
                                focus,
                            },
                            cx,
                        );
                    }
                    Err(err) => {
                        this.fail_core(&core_uuid, || t(M::CheckModsFailed).into(), err, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Play from the instance page, for an instance linked to a server: finds the server
    /// on its core, then plays as the Servers page does (starting it first when it is
    /// stopped and this device may start it).
    pub fn play_instance(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        let Some(link) = self.instance_links.get(instance_id).cloned() else {
            return;
        };
        if cores::friends::is_friend_id(&link.core_uuid) {
            if self.friends.get(&link.core_uuid).is_none() {
                let (name, address) = (link.server_name.clone(), link.core_url.clone());
                return self.fail(
                    move || {
                        (
                            tr!(M::JoinFailed, name = name),
                            tr!(M::FriendRemoved, link = address),
                        )
                    },
                    cx,
                );
            }
            return self.play_friend(&link.core_uuid, cx);
        }
        let Some(index) = self.core_index(&link.core_uuid) else {
            let (name, address) = (link.server_name.clone(), link.core_url.clone());
            return self.fail(
                move || {
                    (
                        tr!(M::JoinFailed, name = name),
                        tr!(M::CoreRemoved, address = address),
                    )
                },
                cx,
            );
        };
        let row: RowKey = (link.core_uuid.clone(), link.server_uuid.clone());
        if self.busy.contains_key(&row) {
            return;
        }
        self.busy.insert(row.clone(), Busy::CheckingMods);
        let core = &self.cores[index];
        let client = self.client(core);
        let key = core.key.clone();
        let work = cx.background_spawn(async move { fetch_list(&client, key.as_ref()).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                let Some(index) = this.core_index(&link.core_uuid) else {
                    return;
                };
                let list = match result {
                    Ok(list) => list,
                    Err(err) => {
                        this.cores[index].status = CoreStatus::Failed(err.clone());
                        let name = link.server_name.clone();
                        return this.fail_core(
                            &link.core_uuid,
                            move || tr!(M::JoinFailed, name = name),
                            err,
                            cx,
                        );
                    }
                };
                let server = list
                    .instances
                    .iter()
                    .find(|s| s.uuid == link.server_uuid)
                    .cloned();
                let signed_in = this.cores[index].signed_in();
                // the core may have a new id now
                let core_uuid = this.listed(index, list, cx);
                let Some(server) = server else {
                    let name = link.server_name.clone();
                    return this.fail(
                        move || (tr!(M::JoinFailed, name = name), t(M::ServerNotShown).into()),
                        cx,
                    );
                };
                match server.state {
                    ServerState::Stopped | ServerState::Error if signed_in => {
                        this.start(&core_uuid, server, true, cx)
                    }
                    ServerState::Stopped | ServerState::Error => {
                        let name = server.name.clone();
                        this.fail(
                            move || {
                                (
                                    tr!(M::NotRunning, name = name),
                                    t(M::AskOwnerToStart).into(),
                                )
                            },
                            cx,
                        )
                    }
                    _ => this.play(&core_uuid, server, cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// The answer to the update prompt: update first, or play as it is.
    pub fn answer_prompt(&mut self, update: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.close(cx) else {
            return;
        };
        let Some(pending) = prompt.pending.take() else {
            cx.notify();
            return;
        };
        if !update {
            let (core, server, instance_id) = (
                prompt.core.clone(),
                prompt.server.clone(),
                prompt.instance_id.clone(),
            );
            let _ = std::fs::remove_file(&pending.pack_path);
            return self.join(&core, &server, &instance_id, cx);
        }
        self.gate(PendingPull::Sync(pending), PullKind::ModSync, cx);
    }

    pub fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        if let Some(pending) = self.prompt.close(cx).and_then(|p| p.pending.take()) {
            let _ = std::fs::remove_file(&pending.pack_path);
            self.friend_cancelled(&pending.core, cx);
        }
        cx.notify();
    }

    /// Brings the instance's mods in step through the modpack update, then joins.
    fn apply_sync(&mut self, pending: PendingSync, cx: &mut Context<Self>) {
        let row: RowKey = (pending.core.clone(), pending.server.uuid.clone());
        if self.instances.read(cx).is_active(&pending.instance_id) {
            let _ = std::fs::remove_file(&pending.pack_path);
            return self.fail(
                || (t(M::UpdateModsFailed).into(), t(M::StopGameFirst).into()),
                cx,
            );
        }
        let instances = self.instances.read(cx);
        let api = instances.modrinth();
        let downloader = instances.content_downloader();
        let dirs = instances.content_dirs(&pending.instance_id);
        let progress = Arc::new(InstallProgress::default());
        self.busy
            .insert(row.clone(), Busy::UpdatingMods(progress.clone()));
        self.tick_while_busy(row.clone(), cx);
        let plan = pending.plan.clone();
        let pack_path = pending.pack_path.clone();
        let etag = pending.etag.clone();
        let work = cx.background_spawn(async move {
            let result = update::apply(
                api.as_deref(),
                &downloader,
                &dirs,
                &plan,
                &pack_path,
                false,
                &progress,
                &CancelToken::new(),
            )
            .await;
            let _ = std::fs::remove_file(&pack_path);
            result?;
            let mut link = link::load_link(&dirs.instance).unwrap_or_default();
            link.pack_etag = etag;
            link::save_link(&dirs.instance, &link).map_err(|e| PullError::Other(e.to_string()))?;
            Ok::<_, PullError>(())
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(()) => {
                        // the loader and Minecraft version follow the server's pack
                        let plan = &pending.plan;
                        let saved = this.instances.update(cx, |instances, cx| {
                            let mut instance = instances
                                .store()
                                .get(&pending.instance_id)
                                .map_err(|e| e.to_string())?;
                            instance.game_version = plan.game_version.clone();
                            instance.loader =
                                Loader::from_kind(plan.loader.as_ref().map(|l| l.kind));
                            instance.loader_version =
                                plan.loader.as_ref().map(|l| l.version.clone());
                            instances.save(&instance, cx)
                        });
                        match saved {
                            Ok(()) => {
                                this.join(&pending.core, &pending.server, &pending.instance_id, cx)
                            }
                            Err(err) => this.fail(
                                move || {
                                    (
                                        t(M::UpdateInstanceFailed).into(),
                                        tr!(M::InstanceNotSaved, error = err),
                                    )
                                },
                                cx,
                            ),
                        }
                    }
                    Err(err) => {
                        this.fail_core(&pending.core, || t(M::UpdateModsFailed).into(), err, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Starts the instance, joining the server.
    fn join(
        &mut self,
        core_uuid: &str,
        server: &ServerInstance,
        instance_id: &str,
        cx: &mut Context<Self>,
    ) {
        if cores::friends::is_friend_id(core_uuid) {
            // the tunnel closed meanwhile (the dialog was closed): nothing to join
            let game_dir = {
                let instances = self.instances.read(cx);
                (!self.preview && !instances.is_active(instance_id))
                    .then(|| instances.game_dir(instance_id))
            };
            let Some(target) = self.friend_join_target(core_uuid, instance_id, game_dir.as_deref())
            else {
                return;
            };
            let id = instance_id.to_string();
            self.friend_started(cx);
            self.instances.update(cx, |instances, cx| {
                instances.play_joining(&id, Some(target), cx)
            });
            cx.emit(ServersEvent::OpenInstance(id));
            // a game that couldn't start closes the tunnel at once
            self.friend_games_changed(cx);
            return;
        }
        let Some(core) = self.cores.iter().find(|c| c.saved.uuid == core_uuid) else {
            return;
        };
        let Some(target) = join_target(&core.url, server) else {
            let (name, shown) = (server.name.clone(), server.address_shown());
            return self.fail(
                move || {
                    (
                        tr!(M::JoinFailed, name = name),
                        t(if shown {
                            M::BadJoinAddress
                        } else {
                            M::AddressHidden
                        })
                        .into(),
                    )
                },
                cx,
            );
        };
        let id = instance_id.to_string();
        self.instances.update(cx, |instances, cx| {
            instances.play_joining(&id, Some(target), cx)
        });
        cx.emit(ServersEvent::OpenInstance(id));
    }

    /// Start, or "Start and join" when `then_join`.
    pub fn start(
        &mut self,
        core_uuid: &str,
        server: ServerInstance,
        then_join: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let Some(key) = self.cores[index].key.clone() else {
            return;
        };
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        self.busy.insert(row.clone(), Busy::Starting);
        let client = self.client(&self.cores[index]);
        let uuid = server.uuid.clone();
        let executor = cx.background_executor().clone();
        let work = cx.background_spawn(async move {
            client.start(&key, &uuid).await?;
            if !then_join {
                return Ok::<_, PullError>(());
            }
            let started = Instant::now();
            loop {
                executor.timer(Duration::from_secs(2)).await;
                let list = client.servers(&key).await?;
                match list
                    .instances
                    .iter()
                    .find(|s| s.uuid == uuid)
                    .map(|s| s.state)
                {
                    Some(ServerState::Running) => return Ok(()),
                    Some(ServerState::Error) => {
                        return Err(PullError::Other(t(M::StartCrashed).into()));
                    }
                    Some(ServerState::Stopped) if started.elapsed() > Duration::from_secs(20) => {
                        return Err(PullError::Other(t(M::StartStopped).into()));
                    }
                    None => return Err(PullError::Other(t(M::ServerGone).into())),
                    _ => {}
                }
                if started.elapsed() > START_TIMEOUT {
                    return Err(PullError::Other(t(M::StartTimedOut).into()));
                }
            }
        });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(()) => {
                        if then_join {
                            let mut running = server.clone();
                            running.state = ServerState::Running;
                            this.play(&core_uuid, running, cx);
                        }
                    }
                    Err(err) => {
                        let name = server.name.clone();
                        this.fail_core(
                            &core_uuid,
                            move || tr!(M::StartFailed, name = name),
                            err,
                            cx,
                        )
                    }
                }
                if let Some(index) = this.core_index(&core_uuid) {
                    this.refresh(index, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn stop(&mut self, core_uuid: &str, server: ServerInstance, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let Some(key) = self.cores[index].key.clone() else {
            return;
        };
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        self.busy.insert(row.clone(), Busy::Stopping);
        let client = self.client(&self.cores[index]);
        let uuid = server.uuid.clone();
        let work = cx.background_spawn(async move { client.stop(&key, &uuid).await });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                if let Err(err) = result {
                    let name = server.name.clone();
                    this.fail_core(&core_uuid, move || tr!(M::StopFailed, name = name), err, cx);
                }
                if let Some(index) = this.core_index(&core_uuid) {
                    this.refresh(index, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Fills the page with made-up cores, for the offscreen screenshots.
    pub fn preview(
        &mut self,
        cores: Vec<(SavedCore, CoreStatus, bool)>,
        links: Vec<(String, String, String)>,
    ) {
        self.preview = true;
        self._poller = None;
        self.local.scanned = true;
        self.cores = cores
            .into_iter()
            .filter_map(|(saved, status, signed_in)| {
                let url = cores::parse_address(&saved.url).ok()?;
                let mut entry =
                    CoreEntry::new(saved, url, signed_in.then(|| Secret::new("preview")));
                entry.status = status;
                Some(entry)
            })
            .collect();
        self.instance_links = links
            .iter()
            .filter_map(|(core, server, instance)| {
                let entry = self.cores.iter().find(|c| &c.saved.uuid == core)?;
                let name = entry
                    .servers()
                    .iter()
                    .find(|s| &s.uuid == server)
                    .map_or_else(|| server.clone(), |s| s.name.clone());
                Some((
                    instance.clone(),
                    ServerLink {
                        core_uuid: core.clone(),
                        core_url: display_address(&entry.url),
                        server_uuid: server.clone(),
                        server_name: name,
                        pack_etag: None,
                    },
                ))
            })
            .collect();
        self.links = links
            .into_iter()
            .map(|(core, server, instance)| ((core, server), instance))
            .collect();
    }

    pub fn preview_busy(&mut self, core: &str, server: &str, busy: Busy) {
        self.busy.insert((core.into(), server.into()), busy);
    }

    pub fn preview_notice(&mut self, title: &str, message: &str) {
        let (title, message) = (title.to_string(), message.to_string());
        self.notice = Some(Notice::new(move || (title.clone(), message.clone())));
    }

    /// Shows "{name} is ready", for the offscreen screenshots.
    pub fn preview_ready(&mut self, name: &str) {
        let name = name.to_string();
        self.notice = Some(Notice::success(move || {
            (
                tr!(M::ReadyTitle, name = name),
                tr!(M::ReadyBody, version = "1.21.1", loader = "Fabric 0.16.14"),
            )
        }));
    }

    pub fn preview_prompt(
        &mut self,
        core: &str,
        server: ServerInstance,
        instance_id: &str,
        changes: PackChanges,
        cx: &mut Context<Self>,
    ) {
        let prompt = UpdatePrompt {
            core: core.into(),
            server,
            instance_id: instance_id.into(),
            changes,
            pending: None,
            focus: cx.focus_handle(),
        };
        self.prompt.open(prompt, cx);
    }

    pub fn preview_advisory(
        &mut self,
        core_name: &str,
        server_name: &str,
        advisory: Advisory,
        cx: &mut Context<Self>,
    ) {
        let prompt = AdvisoryPrompt {
            core_name: core_name.into(),
            server_name: server_name.into(),
            advisory,
            update: false,
            pending: None,
            focus: cx.focus_handle(),
        };
        self.advisory.open(prompt, cx);
    }

    pub fn preview_certificate(
        &mut self,
        index: usize,
        presented: &str,
        previous: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(core) = self.cores.get(index) else {
            return;
        };
        let prompt = CertPrompt {
            address: display_address(&core.url),
            host: core.url.host_str().unwrap_or_default().to_string(),
            name: Some(core.saved.name.clone()),
            presented: presented.into(),
            previous: previous.map(str::to_string),
            target: CertTarget::Core(core.saved.uuid.clone()),
            focus: cx.focus_handle(),
        };
        self.cert_prompt.open(prompt, cx);
    }

    pub fn preview_trust(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(core) = self.cores.get(index) else {
            return;
        };
        let prompt = TrustPrompt {
            core: core.saved.uuid.clone(),
            name: core.saved.name.clone(),
            address: display_address(&core.url),
            after_add: true,
            focus: cx.focus_handle(),
        };
        self.trust_prompt.open(prompt, cx);
    }

    pub fn instances(&self) -> &Entity<Instances> {
        &self.instances
    }
}

enum Check {
    InStep,
    /// The pack on disk, the plan, and the pack's ETag.
    Changed(Box<(PathBuf, UpdatePlan, Option<String>)>),
}

/// Why adding a core stopped.
enum AddFailure {
    /// The certificate needs the user's decision.
    Certificate {
        host: String,
        presented: String,
        previous: Option<String>,
        name: Option<String>,
    },
    Core(CoreError),
}

/// A certificate error while adding a core becomes the certificate prompt, with the name
/// the core gives itself, read through a connection that accepts only the presented
/// certificate (no credentials are sent on it).
async fn certificate_outcome(err: CoreError, http: &Arc<dyn HttpClient>, url: Url) -> AddFailure {
    let (host, presented, previous) = match err {
        CoreError::Untrusted {
            host,
            fingerprint: Some(fingerprint),
        } => (host, fingerprint, None),
        CoreError::CertificateChanged {
            host,
            pinned,
            presented,
        } => (host, presented, Some(pinned)),
        err => return AddFailure::Core(err),
    };
    let name = match pinned_http(Some(&CertificatePin::new(&host, &presented))) {
        Some(pinned) => CoreClient::new(http.clone(), url)
            .with_pinned(pinned)
            .info()
            .await
            .ok()
            .map(|info| info.core_name),
        None => None,
    };
    AddFailure::Certificate {
        host,
        presented,
        previous,
        name,
    }
}

/// Why getting or installing a server's mods failed.
#[derive(Debug)]
pub enum PullError {
    Core(CoreError),
    Cancelled,
    Other(String),
}

impl From<CoreError> for PullError {
    fn from(err: CoreError) -> Self {
        PullError::Core(err)
    }
}

impl From<ModpackError> for PullError {
    fn from(err: ModpackError) -> Self {
        match err {
            ModpackError::Cancelled => PullError::Cancelled,
            err => PullError::Other(err.to_string()),
        }
    }
}

/// In the current language, but for the text of the error itself.
impl std::fmt::Display for PullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PullError::Core(err) => f.write_str(&core_error(err)),
            PullError::Cancelled => f.write_str(t(M::Cancelled)),
            PullError::Other(err) => f.write_str(err),
        }
    }
}

/// The title of a failed "Create matching instance".
fn create_failed() -> String {
    t(M::CreateFailed).into()
}

/// The list a core shows this device: the public one, plus (signed in) every server
/// the key can view.
async fn fetch_list(client: &CoreClient, key: Option<&Secret>) -> Result<ServerList, CoreError> {
    let public = client.public_servers().await;
    let Some(key) = key else {
        return public;
    };
    let mut list = client.servers(key).await?;
    if let Ok(public) = public {
        for server in public.instances {
            if !list.instances.iter().any(|s| s.uuid == server.uuid) {
                list.instances.push(server);
            }
        }
    }
    Ok(list)
}

pub fn create(
    accounts: &Entity<Accounts>,
    instances: &Entity<Instances>,
    cx: &mut App,
) -> Entity<Servers> {
    let accounts = accounts.clone();
    let instances = instances.clone();
    cx.new(|cx| Servers::new(accounts, instances, cx))
}
