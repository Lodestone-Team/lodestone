//! The Servers page: Lodestone cores and their servers, with "Create matching
//! instance", Play (join), Start, "Start and join" and Stop.
//!
//! The cores are kept in `servers.json` (see `launcher_core::cores::store`), each
//! device key in the Keychain. The lists are polled every few seconds while the page
//! is visible.

pub mod add_dialog;
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

use gpui::{App, AppContext as _, Context, Entity, EventEmitter, SharedString, Subscription, Task};
use http_client::HttpClient;
use launcher_core::{
    Secret,
    cores::{
        self, CoreClient, CoreError, PackFetch, ServerInstance, ServerList, ServerState,
        client::display_address,
        join::join_target,
        pack::{self, ClientPack, PackChanges, ServerLink},
        store::{self, SavedCore, ServersFile},
    },
    game::download::{CancelToken, Progress},
};
use url::Url;

use crate::{accounts::Accounts, instances::Instances};
use add_dialog::{AddServerDialog, AddServerEvent};

/// How often the lists are fetched while the page is visible.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// How long "Start and join" waits for the server.
pub const START_TIMEOUT: Duration = Duration::from_secs(5 * 60);

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
}

impl CoreEntry {
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

/// What a server row is doing.
#[derive(Clone, Debug)]
pub enum Busy {
    Creating(Arc<Progress>),
    CheckingMods,
    UpdatingMods(Arc<Progress>),
    Starting,
    Stopping,
}

impl Busy {
    pub fn label(&self) -> String {
        let files = |progress: &Progress, verb: &str| {
            let snapshot = progress.snapshot();
            if snapshot.files_total == 0 {
                format!("{verb}…")
            } else {
                format!(
                    "{verb} mods {}/{}…",
                    snapshot.files_done, snapshot.files_total
                )
            }
        };
        match self {
            Busy::Creating(progress) => files(progress, "Creating instance, downloading"),
            Busy::CheckingMods => "Checking the server's mods…".into(),
            Busy::UpdatingMods(progress) => files(progress, "Updating"),
            Busy::Starting => "Starting the server…".into(),
            Busy::Stopping => "Stopping…".into(),
        }
    }
}

/// Asked before playing when the server's mods changed.
pub struct UpdatePrompt {
    pub core: String,
    pub server: ServerInstance,
    pub instance_id: String,
    pub changes: PackChanges,
    pack: ClientPack,
    etag: Option<String>,
    pub focus: gpui::FocusHandle,
}

pub enum ServersEvent {
    /// Show this launcher instance (it is starting).
    OpenInstance(String),
    /// Show the Servers page.
    ShowServers,
}

/// A row is keyed by core and server.
type RowKey = (String, String);

pub struct Servers {
    accounts: Entity<Accounts>,
    instances: Entity<Instances>,
    http: Arc<dyn HttpClient>,
    file: PathBuf,
    pub cores: Vec<CoreEntry>,
    /// Launcher instances made for a server.
    links: HashMap<RowKey, String>,
    pub busy: HashMap<RowKey, Busy>,
    /// A message at the top of the page.
    pub notice: Option<(SharedString, SharedString)>,
    dialog: Option<(Entity<AddServerDialog>, Subscription)>,
    pub prompt: Option<UpdatePrompt>,
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
            busy: HashMap::new(),
            notice: None,
            dialog: None,
            prompt: None,
            visible: false,
            last_poll: None,
            preview: false,
            tasks: Vec::new(),
            _poller: None,
            extras: Default::default(),
        };
        this.load(cx);
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
                Some(CoreEntry {
                    saved,
                    url,
                    key: None,
                    status: CoreStatus::Loading,
                    refreshing: false,
                })
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
                this.refresh_all(cx);
            })
            .ok();
        }));
    }

    fn save(&mut self) {
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
                        if this.visible {
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
        let root = self.accounts.read(cx).data_dir().instances_dir();
        let existing: HashSet<String> = self
            .instances
            .read(cx)
            .list()
            .iter()
            .map(|i| i.id.clone())
            .collect();
        self.links = pack::linked_instances(&root)
            .into_iter()
            .filter(|(id, _)| existing.contains(id))
            .map(|(id, link)| ((link.core_uuid, link.server_uuid), id))
            .collect();
    }

    fn client(&self, url: &Url) -> CoreClient {
        CoreClient::new(self.http.clone(), url.clone())
    }

    fn refresh(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(core) = self.cores.get_mut(index) else {
            return;
        };
        if core.refreshing {
            return;
        }
        core.refreshing = true;
        let client = CoreClient::new(self.http.clone(), core.url.clone());
        let key = core.key.clone();
        let uuid = core.saved.uuid.clone();
        let fetch = cx.background_spawn(async move { fetch_list(&client, key.as_ref()).await });
        self.tasks.retain(|t| !t.is_ready());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                let Some(index) = this.cores.iter().position(|c| c.saved.uuid == uuid) else {
                    return;
                };
                this.cores[index].refreshing = false;
                match result {
                    Ok(list) => {
                        if this.cores[index].saved.name != list.core_name {
                            this.cores[index].saved.name = list.core_name.clone();
                            this.save();
                        }
                        this.cores[index].status = CoreStatus::Online(list);
                    }
                    Err(CoreError::SignedOut) => this.signed_out(index, cx),
                    Err(err) => this.cores[index].status = CoreStatus::Failed(err),
                }
                cx.notify();
            })
            .ok();
        }));
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
        self.dialog.as_ref().map(|(dialog, _)| dialog)
    }

    /// "Add server", or "Sign in" for a core that is already added (`existing`).
    pub fn open_add(&mut self, existing: Option<usize>, cx: &mut Context<Self>) {
        let prefill = existing
            .and_then(|i| self.cores.get(i))
            .map(|c| display_address(&c.url));
        let dialog = cx.new(|cx| AddServerDialog::new(prefill, cx));
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| match event {
            AddServerEvent::Dismissed => {
                this.dialog = None;
                cx.notify();
            }
            AddServerEvent::Submit(request) => this.add_core(request.clone(), dialog, cx),
        });
        self.dialog = Some((dialog, subscription));
        cx.notify();
    }

    fn add_core(
        &mut self,
        request: add_dialog::AddRequest,
        dialog: Entity<AddServerDialog>,
        cx: &mut Context<Self>,
    ) {
        let url = match cores::parse_address(&request.address) {
            Ok(url) => url,
            Err(err) => {
                dialog.update(cx, |d, cx| d.fail(err.to_string(), cx));
                return;
            }
        };
        let client = self.client(&url);
        let secrets = self.accounts.read(cx).secrets().clone();
        let device = store::device_key_name(&store::host_name());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let credentials = request.username.clone().zip(request.password.clone());
            let expires_at = request.expires_at;
            let result = cx
                .background_spawn(async move {
                    let info = client.info().await?;
                    // an old core has no list: fail now rather than after adding it
                    client.public_servers().await?;
                    let key = match credentials {
                        Some((username, password)) => Some(
                            client
                                .sign_in(&username, &password, &device, expires_at)
                                .await?,
                        ),
                        None => None,
                    };
                    Ok::<_, CoreError>((info, key))
                })
                .await;
            let (info, key) = match result {
                Ok(done) => done,
                Err(err) => {
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
                let saved = SavedCore {
                    uuid: info.uuid.clone(),
                    url: display_address(&url),
                    name: info.core_name.clone(),
                    username: key.as_ref().and(request.username.clone()),
                    key_id: key.as_ref().map(|(_, info)| info.id.clone()),
                    key_expires_at: key.as_ref().and_then(|(_, info)| info.expires_at),
                };
                let entry = CoreEntry {
                    saved,
                    url,
                    key: key.map(|(secret, _)| secret),
                    status: CoreStatus::Loading,
                    refreshing: false,
                };
                let index = match this.cores.iter().position(|c| c.saved.uuid == info.uuid) {
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
                this.dialog = None;
                this.refresh(index, cx);
                cx.notify();
            })
            .ok();
        }));
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
        self.notice = Some((title.into(), message.into()));
        cx.notify();
    }

    pub fn linked_instance(&self, core: &str, server: &str) -> Option<&String> {
        self.links.get(&(core.to_string(), server.to_string()))
    }

    fn core_index(&self, uuid: &str) -> Option<usize> {
        self.cores.iter().position(|c| c.saved.uuid == uuid)
    }

    /// The key to fetch a pack with: this device's key when signed in (any server it
    /// can view), else none (the public route, listed servers only).
    fn key_for(&self, core: &CoreEntry) -> Option<Secret> {
        core.key.clone()
    }

    /// "Create matching instance": a launcher instance with the server's Minecraft
    /// version and loader, with the server's client mods.
    pub fn create_instance(
        &mut self,
        core_uuid: &str,
        server: ServerInstance,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let loader = match cores::matching_loader(&server) {
            Ok(loader) => loader,
            Err(message) => return self.fail("Couldn't create the instance", message, cx),
        };
        let core = &self.cores[index];
        let link = ServerLink {
            core_uuid: core.saved.uuid.clone(),
            core_url: display_address(&core.url),
            server_uuid: server.uuid.clone(),
            server_name: server.name.clone(),
            pack_etag: None,
            files: Vec::new(),
        };
        let created = self.instances.update(cx, |instances, cx| {
            instances.create(&server.name, &server.game_version, loader.as_ref(), cx)
        });
        let instance = match created {
            Ok(instance) => instance,
            Err(message) => return self.fail("Couldn't create the instance", message, cx),
        };
        let instance_dir = self.instances.read(cx).content_dirs(&instance.id).instance;
        if let Err(err) = pack::save_link(&instance_dir, &link) {
            return self.fail("Couldn't create the instance", err.to_string(), cx);
        }
        let key: RowKey = (link.core_uuid.clone(), link.server_uuid.clone());
        self.links.insert(key.clone(), instance.id.clone());
        if !server.has_client_pack {
            cx.notify();
            return;
        }
        let progress = Arc::new(Progress::default());
        self.busy
            .insert(key.clone(), Busy::Creating(progress.clone()));
        self.update_mods(index, server, instance.id, None, progress, false, cx);
    }

    /// Downloads the server's pack (unless given) into the instance, then plays if
    /// `then_play`.
    #[allow(clippy::too_many_arguments)]
    fn update_mods(
        &mut self,
        index: usize,
        server: ServerInstance,
        instance_id: String,
        fetched: Option<(ClientPack, Option<String>)>,
        progress: Arc<Progress>,
        then_play: bool,
        cx: &mut Context<Self>,
    ) {
        let core = &self.cores[index];
        let client = self.client(&core.url);
        let key = self.key_for(core);
        let core_uuid = core.saved.uuid.clone();
        let row: RowKey = (core_uuid.clone(), server.uuid.clone());
        let instances = self.instances.read(cx);
        let Some(modrinth) = instances.modrinth() else {
            self.busy.remove(&row);
            return self.fail("Couldn't download the mods", "The launcher is offline.", cx);
        };
        let downloader = instances.content_downloader();
        let game_dir = instances.game_dir(&instance_id);
        let instance_dir = instances.content_dirs(&instance_id).instance;
        let tick = cx.spawn({
            let row = row.clone();
            async move |this, cx| {
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
            }
        });
        self.tasks.push(tick);
        let server_for_task = server.clone();
        let work = cx.background_spawn(async move {
            let (pack, etag) = match fetched {
                Some(fetched) => fetched,
                None => match client
                    .client_pack(key.as_ref(), &server_for_task, None)
                    .await?
                {
                    PackFetch::Pack { bytes, etag } => {
                        (pack::read_pack(&bytes).map_err(|e| e.to_string())?, etag)
                    }
                    // nothing to install
                    PackFetch::NoMods(_) | PackFetch::NotModified => return Ok(None),
                },
            };
            let previous = pack::load_link(&instance_dir);
            let files = pack::apply(
                &pack,
                &game_dir,
                previous.as_ref(),
                &modrinth,
                &downloader,
                &progress,
                &CancelToken::new(),
            )
            .await
            .map_err(|e| e.to_string())?;
            let mut link = previous.unwrap_or_default();
            link.pack_etag = etag;
            link.files = files;
            pack::save_link(&instance_dir, &link).map_err(|e| e.to_string())?;
            Ok::<_, UpdateError>(Some(link.files.len()))
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(_) if then_play => this.join(&core_uuid, &server, &instance_id, cx),
                    Ok(_) => {}
                    Err(UpdateError::Core(CoreError::SignedOut)) => {
                        if let Some(index) = this.core_index(&core_uuid) {
                            this.signed_out(index, cx);
                        }
                    }
                    Err(err) => this.fail("Couldn't get the server's mods", err.to_string(), cx),
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
        if !server.has_client_pack {
            return self.join(core_uuid, &server, &instance_id, cx);
        }
        let row: RowKey = (core_uuid.to_string(), server.uuid.clone());
        self.busy.insert(row.clone(), Busy::CheckingMods);
        let core = &self.cores[index];
        let client = self.client(&core.url);
        let key = self.key_for(core);
        let instances = self.instances.read(cx);
        let instance_dir = instances.content_dirs(&instance_id).instance;
        let mods_dir = instances.game_dir(&instance_id).join("mods");
        let check_server = server.clone();
        let check = cx.background_spawn(async move {
            let link = pack::load_link(&instance_dir).unwrap_or_default();
            let fetched = client
                .client_pack(key.as_ref(), &check_server, link.pack_etag.as_deref())
                .await?;
            let on_disk = pack::changed_on_disk(&link, &mods_dir);
            Ok::<_, UpdateError>(match fetched {
                PackFetch::NotModified if on_disk.is_empty() => Check::InStep,
                PackFetch::NoMods(_) => Check::InStep,
                PackFetch::NotModified => Check::Refetch,
                PackFetch::Pack { bytes, etag } => {
                    let pack = pack::read_pack(&bytes).map_err(|e| e.to_string())?;
                    let changes = pack::changes(Some(&link), &pack);
                    if changes.is_empty() && on_disk.is_empty() {
                        // same mods in a rebuilt pack: remember its ETag
                        let mut link = link;
                        link.pack_etag = etag;
                        let _ = pack::save_link(&instance_dir, &link);
                        Check::InStep
                    } else {
                        let mut changes = changes;
                        for name in on_disk {
                            if !changes.updated.contains(&name) && !changes.removed.contains(&name)
                            {
                                changes.updated.push(name);
                            }
                        }
                        Check::Changed(Box::new((pack, etag, changes)))
                    }
                }
            })
        });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = check.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(Check::InStep) => this.join(&core_uuid, &server, &instance_id, cx),
                    Ok(Check::Refetch) => {
                        // files changed on disk: get the whole pack to compare
                        if let Some(index) = this.core_index(&core_uuid) {
                            let progress = Arc::new(Progress::default());
                            this.busy
                                .insert(row.clone(), Busy::UpdatingMods(progress.clone()));
                            this.update_mods(index, server, instance_id, None, progress, true, cx);
                        }
                    }
                    Ok(Check::Changed(boxed)) => {
                        let (pack, etag, changes) = *boxed;
                        this.prompt = Some(UpdatePrompt {
                            core: core_uuid.clone(),
                            server,
                            instance_id,
                            changes,
                            pack,
                            etag,
                            focus: cx.focus_handle(),
                        });
                    }
                    Err(UpdateError::Core(CoreError::SignedOut)) => {
                        if let Some(index) = this.core_index(&core_uuid) {
                            this.signed_out(index, cx);
                        }
                    }
                    Err(err) => this.fail("Couldn't check the server's mods", err.to_string(), cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// The answer to the update prompt: update first, or play as it is.
    pub fn answer_prompt(&mut self, update: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        if !update {
            return self.join(&prompt.core, &prompt.server, &prompt.instance_id, cx);
        }
        let Some(index) = self.core_index(&prompt.core) else {
            return;
        };
        let progress = Arc::new(Progress::default());
        self.busy.insert(
            (prompt.core.clone(), prompt.server.uuid.clone()),
            Busy::UpdatingMods(progress.clone()),
        );
        self.update_mods(
            index,
            prompt.server,
            prompt.instance_id,
            Some((prompt.pack, prompt.etag)),
            progress,
            true,
            cx,
        );
    }

    pub fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        self.prompt = None;
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
                "Couldn't join the server",
                "The server's address or port isn't one Minecraft can join.",
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
        let client = self.client(&self.cores[index].url);
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
                    Err(CoreError::SignedOut) => {
                        if let Some(index) = this.core_index(&core_uuid) {
                            this.signed_out(index, cx);
                        }
                    }
                    Err(err) => this.fail(
                        format!("Couldn't start {}", server.name),
                        err.to_string(),
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
        let client = self.client(&self.cores[index].url);
        let uuid = server.uuid.clone();
        let work = cx.background_spawn(async move { client.stop(&key, &uuid).await });
        let core_uuid = core_uuid.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.busy.remove(&row);
                match result {
                    Ok(()) => {}
                    Err(CoreError::SignedOut) => {
                        if let Some(index) = this.core_index(&core_uuid) {
                            this.signed_out(index, cx);
                        }
                    }
                    Err(err) => this.fail(
                        format!("Couldn't stop {}", server.name),
                        err.to_string(),
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
                Some(CoreEntry {
                    url: cores::parse_address(&saved.url).ok()?,
                    saved,
                    key: signed_in.then(|| Secret::new("preview")),
                    status,
                    refreshing: false,
                })
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
        self.prompt = Some(UpdatePrompt {
            core: core.into(),
            server,
            instance_id: instance_id.into(),
            changes,
            pack: ClientPack {
                name: String::new(),
                game_version: None,
                fabric_loader: None,
                mods: Vec::new(),
                skipped: Vec::new(),
            },
            etag: None,
            focus: cx.focus_handle(),
        });
    }

    pub fn instances(&self) -> &Entity<Instances> {
        &self.instances
    }
}

enum Check {
    InStep,
    /// The ETag matched, but files changed on disk.
    Refetch,
    Changed(Box<(ClientPack, Option<String>, PackChanges)>),
}

/// Why getting a server's mods failed.
#[derive(Debug)]
enum UpdateError {
    Core(CoreError),
    Other(String),
}

impl From<CoreError> for UpdateError {
    fn from(err: CoreError) -> Self {
        UpdateError::Core(err)
    }
}

impl From<String> for UpdateError {
    fn from(err: String) -> Self {
        UpdateError::Other(err)
    }
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Core(err) => err.fmt(f),
            UpdateError::Other(err) => f.write_str(err),
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
