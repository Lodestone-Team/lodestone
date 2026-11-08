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
pub mod host;
pub mod lan;
pub mod tools;
pub mod view;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
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
        self, CoreClient, CoreError, PackFetch, ServerInstance, ServerList, ServerState,
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

use crate::{accounts::Accounts, components::DialogSlot, instances::Instances};
use add_dialog::{AddRequest, AddServerDialog, AddServerEvent};

/// How often the lists are fetched while the page is visible.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// How long "Start and join" waits for the server.
pub const START_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// What trusting a core for mods means, shown wherever the user is asked.
pub const TRUST_EXPLANATION: &str = "Mods from a Lodestone core run as programs on this Mac, with the same access to your files and accounts as you. Only trust a core whose owner you trust.";

/// Where to find a core's certificate fingerprint, to compare.
pub const FINGERPRINT_HELP: &str = "Compare it with the fingerprint the core shows: in the dashboard under Settings → Core Settings → Information (\"Certificate fingerprint (SHA-256)\"), or in the core's log when it starts (\"TLS certificate SHA-256 fingerprint\").";

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
        let files = |progress: &InstallProgress, verb: &str| {
            let snapshot = progress.downloads.snapshot();
            if snapshot.files_total == 0 {
                format!("{verb}…")
            } else {
                format!(
                    "{verb} {}/{} files…",
                    snapshot.files_done, snapshot.files_total
                )
            }
        };
        match self {
            Busy::Fetching => "Getting the server's mods…".into(),
            Busy::Creating(progress) => files(progress, "Creating instance, downloading"),
            Busy::CheckingMods => "Checking the server's mods…".into(),
            Busy::UpdatingMods(progress) => files(progress, "Updating mods,"),
            Busy::Starting => "Starting the server…".into(),
            Busy::Stopping => "Stopping…".into(),
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
    pub busy: HashMap<RowKey, Busy>,
    /// A message at the top of the page.
    pub notice: Option<(SharedString, SharedString)>,
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
    /// World downloads, resource packs, hosting and LAN worlds.
    extras: extras::Extras,
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
        };
        this.load(cx);
        this.reload_links(cx);
        this
    }

    /// Reads `servers.json` and the keys.
    fn load(&mut self, cx: &mut Context<Self>) {
        let file = match store::load(&self.file) {
            Ok(file) => file,
            Err(err) => {
                self.notice = Some(("Couldn't read your servers".into(), err.to_string().into()));
                ServersFile::default()
            }
        };
        self.cores = file
            .cores
            .into_iter()
            .filter_map(|saved| {
                let url = cores::parse_address(&saved.url).ok()?;
                Some(CoreEntry::new(saved, url, None))
            })
            .collect();
        let secrets = self.accounts.read(cx).secrets().clone();
        let signed_in: Vec<String> = self
            .cores
            .iter()
            .filter(|c| c.saved.signed_in())
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
            self.notice = Some(("Couldn't save your servers".into(), err.to_string().into()));
        }
    }

    /// The page is shown or hidden; lists are polled only while it is shown.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.lan_visible(visible, cx);
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
        self.reload_links(cx);
        for index in 0..self.cores.len() {
            self.refresh(index, cx);
        }
    }

    /// Finds the launcher instances made for a server.
    fn reload_links(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let root = self.accounts.read(cx).data_dir().instances_dir();
        let existing: HashSet<String> = self
            .instances
            .read(cx)
            .list()
            .iter()
            .map(|i| i.id.clone())
            .collect();
        let linked: Vec<(String, ServerLink)> = link::linked_instances(&root)
            .into_iter()
            .filter(|(id, _)| existing.contains(id))
            .collect();
        self.links = linked
            .iter()
            .map(|(id, link)| {
                (
                    (link.core_uuid.clone(), link.server_uuid.clone()),
                    id.clone(),
                )
            })
            .collect();
        self.instance_links = linked.into_iter().collect();
    }

    fn client(&self, core: &CoreEntry) -> CoreClient {
        let client = CoreClient::new(self.http.clone(), core.url.clone());
        match &core.pinned {
            Some(pinned) => client.with_pinned(pinned.clone()),
            None => client,
        }
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
                    Ok(list) => this.listed(index, list),
                    Err(CoreError::SignedOut) => this.signed_out(index, cx),
                    Err(err) => this.cores[index].status = CoreStatus::Failed(err),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// A list arrived: keep the name, and end trust when another core answers.
    fn listed(&mut self, index: usize, list: ServerList) {
        let core = &mut self.cores[index];
        let mut changed = false;
        if core.saved.name != list.core_name {
            core.saved.name = list.core_name.clone();
            changed = true;
        }
        if !list.core_uuid.is_empty() && core.saved.check_identity(&list.core_uuid) {
            changed = true;
            self.notice = Some((
                format!("{} is no longer trusted for mods", core.saved.name).into(),
                format!(
                    "Another core answers at {} now (its id changed). Trust it again on its card only if you trust its owner.",
                    display_address(&core.url)
                )
                .into(),
            ));
        }
        self.cores[index].status = CoreStatus::Online(list);
        if changed {
            self.save();
        }
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
        self.notice = Some((
            format!("Signed out of {name}").into(),
            CoreError::SignedOut.to_string().into(),
        ));
        self.forget_key(uuid, cx);
        self.refresh(index, cx);
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
                dialog.update(cx, |d, cx| d.fail(err.to_string(), cx));
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
                        dialog.update(cx, |d, cx| {
                            d.fail(
                                "Check the core's certificate to continue.".into(),
                                cx,
                            )
                        });
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
                    dialog.update(cx, |d, cx| d.fail(err.to_string(), cx));
                    return;
                }
            };
            if let Some((secret, _)) = &key {
                let store = secrets.store(cx);
                if let Err(err) = store::write_key(&*store, &info.uuid, secret).await {
                    dialog.update(cx, |d, cx| {
                        d.fail(
                            format!("Couldn't save the key in the Keychain: {err:#}"),
                            cx,
                        )
                    });
                    return;
                }
            }
            this.update(cx, |this, cx| {
                let existing = this.core_index(&info.uuid);
                // another core used to answer at this address
                for core in &mut this.cores {
                    if display_address(&core.url) == address
                        && core.saved.uuid != info.uuid
                        && core.saved.check_identity(&info.uuid)
                    {
                        this.notice = Some((
                            format!("{} is no longer trusted for mods", core.saved.name).into(),
                            format!("Another core answers at {address} now.").into(),
                        ));
                    }
                }
                let mut saved = match existing {
                    Some(index) => this.cores[index].saved.clone(),
                    None => SavedCore::default(),
                };
                let was_trusted = saved.is_trusted();
                saved.uuid = info.uuid.clone();
                saved.url = address.clone();
                saved.name = info.core_name.clone();
                if key.is_some() {
                    saved.username = request.username.clone();
                    saved.key_id = key.as_ref().map(|(_, info)| info.id.clone());
                    saved.key_expires_at = key.as_ref().and_then(|(_, info)| info.expires_at);
                }
                if let Some(pin) = pin.clone()
                    && saved.pin_certificate(pin)
                {
                    this.notice = Some((
                        format!("{} is no longer trusted for mods", saved.name).into(),
                        "You trusted a new certificate for it. Trust it for mods again on its card if you're sure it is the same core.".into(),
                    ));
                }
                let entry = CoreEntry::new(
                    saved,
                    url,
                    key.map(|(secret, _)| secret)
                        .or_else(|| existing.and_then(|i| this.cores[i].key.clone())),
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
                    dialog.update(cx, |d, cx| {
                        d.fail(
                            "The certificate wasn't trusted, so your sign-in wasn't sent to the core.".into(),
                            cx,
                        )
                    });
                }
            }
            CertTarget::Core(uuid) if trust => {
                let Some(index) = self.core_index(&uuid) else {
                    return;
                };
                let core = &mut self.cores[index];
                if core.saved.pin_certificate(pin) {
                    self.notice = Some((
                        format!("{} is no longer trusted for mods", core.saved.name).into(),
                        "Its certificate changed. Trust it for mods again on its card if you're sure it is the same core.".into(),
                    ));
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
        let Some(core) = self.cores.get_mut(index) else {
            return;
        };
        let name = core.saved.name.clone();
        let key_id = core.saved.key_id.take();
        core.key = None;
        core.saved.username = None;
        core.saved.key_expires_at = None;
        let uuid = core.saved.uuid.clone();
        self.save();
        self.forget_key(uuid, cx);
        if let Some(id) = key_id {
            self.notice = Some((
                format!("Signed out of {name}").into(),
                format!(
                    "The key for this device (lsk_{id}_…) is still valid until it expires. Revoke it under Settings → API keys in the dashboard to end it now."
                )
                .into(),
            ));
        }
        self.refresh(index, cx);
        cx.notify();
    }

    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.cores.len() {
            return;
        }
        let core = self.cores.remove(index);
        self.save();
        if core.signed_in() {
            self.forget_key(core.saved.uuid, cx);
        }
        cx.notify();
    }

    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }

    fn fail(
        &mut self,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let (title, message) = (title.into(), message.into());
        if !self.visible {
            cx.emit(ServersEvent::Failed {
                title: title.clone(),
                message: message.clone(),
            });
        }
        self.notice = Some((title, message));
        cx.notify();
    }

    /// A failure of a request to a core: the key stopped working, or something else.
    fn fail_core(
        &mut self,
        core: &str,
        title: &str,
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
            err => self.fail(title.to_string(), err.to_string(), cx),
        }
    }

    pub fn linked_instance(&self, core: &str, server: &str) -> Option<&String> {
        self.links.get(&(core.to_string(), server.to_string()))
    }

    /// The server an instance is linked to, for the instance page.
    pub fn instance_link(&self, instance_id: &str) -> Option<InstanceLink> {
        let link = self.instance_links.get(instance_id)?;
        let core = self.cores.iter().find(|c| c.saved.uuid == link.core_uuid);
        Some(InstanceLink {
            server_name: link.server_name.clone(),
            core_name: core.map_or_else(|| link.core_url.clone(), |c| c.saved.name.clone()),
            core_known: core.is_some(),
        })
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
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        if !server.has_pack() {
            return self.create_plain(core_uuid, &server, cx);
        }
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        self.busy.insert(row.clone(), Busy::Fetching);
        let core = &self.cores[index];
        let client = self.client(core);
        let key = core.key.clone();
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
                    Err(err) => this.fail_core(&core_uuid, "Couldn't create the instance", err, cx),
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
            Err(message) => return self.fail("Couldn't create the instance", message, cx),
        };
        let created = self.instances.update(cx, |instances, cx| {
            instances.create(&server.name, &server.game_version, loader.as_ref(), cx)
        });
        match created {
            Ok(instance) => {
                self.link_instance(core_uuid, server, &instance.id, None, cx);
                if server.mods_hidden() {
                    self.notice = Some((
                        format!("Created {} without the server's mods", server.name).into(),
                        "The owner hasn't shared this server's client mods, so the instance has only its Minecraft version and loader. Add any mods it needs yourself.".into(),
                    ));
                }
            }
            Err(message) => self.fail("Couldn't create the instance", message, cx),
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
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let link = ServerLink {
            core_uuid: core_uuid.to_string(),
            core_url: display_address(&self.cores[index].url),
            server_uuid: server.uuid.clone(),
            server_name: server.name.clone(),
            pack_etag: etag,
        };
        let instance_dir = self.instances.read(cx).content_dirs(instance_id).instance;
        if let Err(err) = link::save_link(&instance_dir, &link) {
            return self.fail(
                "Couldn't link the instance to the server",
                err.to_string(),
                cx,
            );
        }
        self.links.insert(
            (link.core_uuid.clone(), link.server_uuid.clone()),
            instance_id.to_string(),
        );
        self.instance_links.insert(instance_id.to_string(), link);
        cx.notify();
    }

    /// Installs `pending` from its core, after the advisory unless the user trusts the
    /// core for mods.
    fn gate(&mut self, pending: PendingPull, kind: PullKind, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(pending.core()) else {
            pending.discard();
            return;
        };
        if !trust::needs_advisory(&self.cores[index].saved, kind) {
            return self.install(pending, cx);
        }
        let files: Vec<PulledFile> = match &pending {
            PendingPull::Create(p) => sync::pack_pulls(&p.pack),
            PendingPull::Sync(p) => sync::plan_pulls(&p.plan),
        };
        let core_name = self.cores[index].saved.name.clone();
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
            AdvisoryAnswer::Cancel => pending.discard(),
            AdvisoryAnswer::InstallOnce => self.install(pending, cx),
            AdvisoryAnswer::TrustAndInstall => {
                if let Some(index) = self.core_index(pending.core()) {
                    self.cores[index].saved.set_trusted(true);
                    self.save();
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
                        this.link_instance(
                            &pending.core,
                            &pending.server,
                            &instance.id,
                            pending.etag.clone(),
                            cx,
                        );
                        let skipped = match pending.pack.skipped.len() {
                            0 => String::new(),
                            n => format!(" {n} file(s) of the pack were left out."),
                        };
                        this.notice = Some((
                            format!("{} is ready", instance.name).into(),
                            format!(
                                "Minecraft {} with {}, and the server's mods and configs.{skipped} Press Play to join.",
                                instance.game_version,
                                instance.loader_label()
                            )
                            .into(),
                        ));
                    }
                    Err(err) => {
                        this.fail_core(&pending.core, "Couldn't create the instance", err, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Play: checks that the instance's mods match the server's, then joins.
    pub fn play(&mut self, core_uuid: &str, server: ServerInstance, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(core_uuid) else {
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
        let core = &self.cores[index];
        let client = self.client(core);
        let key = core.key.clone();
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
                        this.fail_core(&core_uuid, "Couldn't check the server's mods", err, cx)
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
        let Some(index) = self.core_index(&link.core_uuid) else {
            return self.fail(
                format!("Couldn't join {}", link.server_name),
                format!(
                    "The core this instance was made for ({}) isn't in Lodestone Launcher any more. Add it on the Servers page, or press Play without joining.",
                    link.core_url
                ),
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
                        return this.fail_core(
                            &link.core_uuid,
                            &format!("Couldn't join {}", link.server_name),
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
                this.listed(index, list);
                let Some(server) = server else {
                    return this.fail(
                        format!("Couldn't join {}", link.server_name),
                        "The core doesn't show this server to you any more: it was deleted, or it isn't listed publicly and you aren't signed in.",
                        cx,
                    );
                };
                match server.state {
                    ServerState::Stopped | ServerState::Error if signed_in => {
                        this.start(&link.core_uuid, server, true, cx)
                    }
                    ServerState::Stopped | ServerState::Error => this.fail(
                        format!("{} isn't running", server.name),
                        "Ask its owner to start it, or sign in to the core on the Servers page to start it yourself.",
                        cx,
                    ),
                    _ => this.play(&link.core_uuid, server, cx),
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
        }
        cx.notify();
    }

    /// Brings the instance's mods in step through the modpack update, then joins.
    fn apply_sync(&mut self, pending: PendingSync, cx: &mut Context<Self>) {
        let row: RowKey = (pending.core.clone(), pending.server.uuid.clone());
        if self.instances.read(cx).is_active(&pending.instance_id) {
            let _ = std::fs::remove_file(&pending.pack_path);
            return self.fail("Couldn't update the mods", "Stop the game first.", cx);
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
                            Ok(()) => this.join(
                                &pending.core,
                                &pending.server,
                                &pending.instance_id,
                                cx,
                            ),
                            Err(err) => this.fail(
                                "Couldn't update the instance",
                                format!("The mods were updated, but the instance couldn't be saved: {err}"),
                                cx,
                            ),
                        }
                    }
                    Err(err) => this.fail_core(&pending.core, "Couldn't update the mods", err, cx),
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
        let Some(core) = self.cores.iter().find(|c| c.saved.uuid == core_uuid) else {
            return;
        };
        let Some(target) = join_target(&core.url, server) else {
            return self.fail(
                format!("Couldn't join {}", server.name),
                if server.address_shown() {
                    "The server's address or port isn't one Minecraft can join."
                } else {
                    cores::models::ADDRESS_HIDDEN
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
                return Ok(());
            }
            let started = Instant::now();
            loop {
                executor.timer(Duration::from_secs(2)).await;
                let list = client.servers(&key).await?;
                match list.instances.iter().find(|s| s.uuid == uuid).map(|s| s.state) {
                    Some(ServerState::Running) => return Ok(()),
                    Some(ServerState::Error) => {
                        return Err(CoreError::Server {
                            status: 500,
                            message: "The server crashed while starting. Check its console in the dashboard.".into(),
                        });
                    }
                    Some(ServerState::Stopped) if started.elapsed() > Duration::from_secs(20) => {
                        return Err(CoreError::Server {
                            status: 500,
                            message: "The server stopped while starting. Check its console in the dashboard.".into(),
                        });
                    }
                    None => return Err(CoreError::NotFound("The server is gone.".into())),
                    _ => {}
                }
                if started.elapsed() > START_TIMEOUT {
                    return Err(CoreError::Server {
                        status: 504,
                        message: "The server didn't finish starting within 5 minutes.".into(),
                    });
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
                    Err(err) => this.fail_core(
                        &core_uuid,
                        &format!("Couldn't start {}", server.name),
                        err,
                        cx,
                    ),
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
                    this.fail_core(
                        &core_uuid,
                        &format!("Couldn't stop {}", server.name),
                        err,
                        cx,
                    );
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
        self.notice = Some((title.to_string().into(), message.to_string().into()));
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

impl std::fmt::Display for PullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PullError::Core(err) => err.fmt(f),
            PullError::Cancelled => f.write_str("Cancelled"),
            PullError::Other(err) => f.write_str(err),
        }
    }
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
