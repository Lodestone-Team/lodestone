//! The control API: what `lodestone-launcher-cli` (and other programs of this user)
//! can do with the running launcher, over a Unix domain socket in the data directory
//! (on Windows too, which has had them since Windows 10 1803).
//!
//! The socket and its threads are `launcher_control`'s. They only read and write the
//! socket: every request comes here, to the GPUI main thread, and is handled with the
//! app's own entities ([`Instances`], [`Accounts`], [`Servers`]) and the same
//! `launcher_core` calls the window makes, so the window shows the change at once and
//! the two never disagree.
//!
//! Rules kept here:
//!
//! - no method returns a secret: no Microsoft token, core key or password (passwords
//!   come in, to sign in to a core, and are dropped after);
//! - destructive methods need `"confirm": true`;
//! - installing a core's mods keeps the trust advisory: a core that isn't trusted for
//!   mods gets an `untrusted_core` error with the advisory, and trusting it is its own
//!   method (`cores.trust`) or flag;
//! - hosting needs `"accept_eula": true`, the EULA box of the dialog.
//!
//! The method table is [`METHODS`]; `meta.methods` returns it.

mod accounts;
mod content;
mod events;
mod import;
mod instances;
mod jobs;
mod perf;
mod servers;
mod settings;
mod skins;
mod snapshots;
mod storage;
mod trace;
mod worlds;

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures::StreamExt as _;
use gpui::{App, AppContext as _, AsyncApp, Entity, Subscription, Task};
use launcher_control::{
    ErrorObject, PROTOCOL_VERSION, Request, codes, paths,
    server::{Backend, Outbox, Server, ServerHandle},
};
use launcher_core::{
    cores::CoreError,
    instances::Instance,
    settings::{CliAccess, CliSettings},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::{
    accounts::Accounts,
    instances::{Instances, content::ContentView, worlds::WorldsView},
    servers::Servers,
    shell::LauncherShell,
};

pub use jobs::{JobState, Jobs};

pub type ApiResult = Result<Value, ErrorObject>;
/// What a handler returns: the response, now or later.
pub type Reply = Task<ApiResult>;

/// An error with a code from [`codes`].
pub fn api_error(code: &str, message: impl Into<String>) -> ErrorObject {
    ErrorObject::new(code, message)
}

/// A core's error, with the code that says what to do about it. Core errors never hold
/// a password, token or key.
pub fn core_error(err: CoreError) -> ErrorObject {
    let code = match &err {
        CoreError::InvalidAddress(_) => codes::INVALID_PARAMS,
        CoreError::Unreachable(_) => codes::UNAVAILABLE,
        CoreError::Untrusted { .. } | CoreError::CertificateChanged { .. } => {
            codes::UNTRUSTED_CERTIFICATE
        }
        CoreError::SignedOut => codes::NOT_SIGNED_IN,
        CoreError::BadCredentials | CoreError::Forbidden(_) => codes::PERMISSION_DENIED,
        CoreError::NotFound(_) => codes::NOT_FOUND,
        CoreError::TooOld => codes::UNAVAILABLE,
        _ => codes::FAILED,
    };
    let error = api_error(code, err.to_string());
    match err {
        CoreError::Untrusted {
            host,
            fingerprint: Some(fingerprint),
        } => error.with_data(json!({ "host": host, "fingerprint": fingerprint })),
        CoreError::CertificateChanged {
            host,
            pinned,
            presented,
        } => error.with_data(json!({ "host": host, "fingerprint": presented, "previous": pinned })),
        _ => error,
    }
}

/// One method of the API.
pub struct Method {
    pub name: &'static str,
    pub summary: &'static str,
    /// Needs `"confirm": true`.
    pub destructive: bool,
    pub handler: fn(Call, &mut App) -> Reply,
}

const fn method(
    name: &'static str,
    summary: &'static str,
    handler: fn(Call, &mut App) -> Reply,
) -> Method {
    Method {
        name,
        summary,
        destructive: false,
        handler,
    }
}

const fn destructive(
    name: &'static str,
    summary: &'static str,
    handler: fn(Call, &mut App) -> Reply,
) -> Method {
    Method {
        name,
        summary,
        destructive: true,
        handler,
    }
}

/// Every method, in the order `meta.methods` lists them.
pub const METHODS: &[Method] = &[
    method(
        "meta.ping",
        "The launcher's version and protocol version.",
        meta_ping,
    ),
    method(
        "meta.methods",
        "Every method with its summary.",
        meta_methods,
    ),
    method(
        "meta.status",
        "The whole launcher at a glance: instances, runs, jobs, accounts, cores.",
        meta_status,
    ),
    method(
        "meta.hello",
        "Says which program is connected (client, pid), for the launcher's Settings > Developer list. Answers with the access level.",
        meta_hello,
    ),
    // instances
    method(
        "instances.list",
        "Instances with their effective sidebar group and state.",
        instances::list,
    ),
    method(
        "instances.show",
        "One instance, with its settings and what it is doing.",
        instances::show,
    ),
    method(
        "instances.select",
        "Selects the instance in the window.",
        instances::select,
    ),
    method(
        "instances.versions",
        "Minecraft versions to make an instance with.",
        instances::versions,
    ),
    method(
        "instances.loader_versions",
        "A mod loader's versions for a Minecraft version.",
        instances::loader_versions,
    ),
    method(
        "instances.create",
        "Makes an instance (vanilla or with a mod loader).",
        instances::create,
    ),
    method(
        "instances.rename",
        "Renames an instance.",
        instances::rename,
    ),
    method(
        "instances.set_group",
        "Puts an instance in a sidebar group, or none.",
        instances::set_group,
    ),
    method(
        "instances.set_prerelease_updates",
        "Lets an instance's content update to beta and alpha versions (on), or only to versions at least as stable as installed (off).",
        instances::set_prerelease_updates,
    ),
    method(
        "instances.set_icon",
        "Sets an instance's icon: an image, a library icon, an emoji, or none.",
        instances::set_icon,
    ),
    method(
        "instances.set_background",
        "Sets or clears an instance's Play background.",
        instances::set_background,
    ),
    method(
        "instances.duplicate",
        "Copies an instance.",
        instances::duplicate,
    ),
    destructive(
        "instances.delete",
        "Moves an instance to the Trash.",
        instances::delete,
    ),
    method(
        "instances.export",
        "Writes an instance as a Modrinth modpack (.mrpack).",
        instances::export,
    ),
    method(
        "instances.share_code",
        "An instance's Modrinth mods, resource packs and shaders as a share code another Lodestone Launcher can paste. Lists what it leaves out (CurseForge and hand-added files).",
        content::share_code,
    ),
    method(
        "instances.import_code",
        "Makes an instance from a share code, then installs its content from Modrinth (a job).",
        content::import_code,
    ),
    method(
        "instances.verify",
        "Checks every game file and downloads missing or broken ones.",
        instances::verify,
    ),
    // play
    method(
        "play.launch",
        "Starts the game (joining the linked server, or a given address).",
        instances::launch,
    ),
    method(
        "play.stop",
        "Stops the game, or cancels its preparation.",
        instances::stop,
    ),
    method(
        "play.status",
        "What each instance's game is doing.",
        instances::status,
    ),
    method("play.logs", "An instance's console rows.", instances::logs),
    destructive(
        "play.clear_logs",
        "Clears an instance's console.",
        instances::clear_logs,
    ),
    // settings
    method(
        "settings.instance.get",
        "An instance's settings: its own and the defaults they follow.",
        settings::instance_get,
    ),
    method(
        "settings.instance.set",
        "Changes an instance's settings.",
        settings::instance_set,
    ),
    method(
        "settings.instance.reset",
        "Makes instance settings follow the launcher defaults again.",
        settings::instance_reset,
    ),
    method(
        "settings.defaults.get",
        "The launcher's instance defaults.",
        settings::defaults_get,
    ),
    method(
        "settings.defaults.set",
        "Changes the launcher's instance defaults.",
        settings::defaults_set,
    ),
    method(
        "settings.defaults.reset",
        "Resets launcher defaults to the built-in values.",
        settings::defaults_reset,
    ),
    method(
        "settings.general.get",
        "General settings, such as automatic sidebar groups.",
        settings::general_get,
    ),
    method(
        "settings.general.set",
        "Changes general settings.",
        settings::general_set,
    ),
    method(
        "skins.list",
        "The launcher's skins (built-in and imported packs), and the one in use.",
        skins::list,
    ),
    method(
        "skins.use",
        "Uses a skin by id (\"default\" for the default look).",
        skins::use_skin,
    ),
    method(
        "skins.import",
        "Imports a skin pack from a .zip file or a folder (absolute path); use: true to use it.",
        skins::import,
    ),
    method(
        "skins.export",
        "Writes a skin (the one in use without id) to a .zip file (absolute path).",
        skins::export,
    ),
    method(
        "skins.check",
        "Readability checks for a skin (the one in use without id), or a pack at path (absolute; checked as importing would, not kept): each surface's text, its contrast, the target and a suggested fix.",
        skins::check,
    ),
    method(
        "skins.fix",
        "Applies the suggested readability fixes to an imported skin (the one in use without id), a round at a time; not to a built-in skin.",
        skins::fix,
    ),
    method(
        "storage.show",
        "What the data folder takes: instances, game files, Java runtimes, cache, logs; and what is unused.",
        storage::show,
    ),
    destructive(
        "storage.clean",
        "Removes unused game files or Java runtimes, the cache, old logs or all logs (what: game-files, runtimes, cache, leftovers, old-logs, logs).",
        storage::clean,
    ),
    method(
        "settings.developer.get",
        "Developer settings.",
        settings::developer_get,
    ),
    method(
        "settings.developer.set",
        "Changes developer settings.",
        settings::developer_set,
    ),
    method(
        "shared.get",
        "Shared game settings: categories, values, the instance they sync from.",
        settings::shared_get,
    ),
    method(
        "shared.set_value",
        "Sets one shared game setting.",
        settings::shared_set_value,
    ),
    method(
        "shared.set_category",
        "Shares a category of game settings, or stops sharing it.",
        settings::shared_set_category,
    ),
    method(
        "shared.sync_from",
        "The instance whose in-game changes become the shared values.",
        settings::shared_sync_from,
    ),
    method(
        "shared.copy_from",
        "Copies an instance's current game settings into the shared values.",
        settings::shared_copy_from,
    ),
    destructive(
        "shared.clear",
        "Forgets every shared value.",
        settings::shared_clear,
    ),
    method(
        "shared.instance.get",
        "An instance's shared settings: on or off, its overrides.",
        settings::shared_instance_get,
    ),
    method(
        "shared.instance.set",
        "Turns sharing on or off for an instance, or overrides keys.",
        settings::shared_instance_set,
    ),
    // content
    method(
        "content.list",
        "Installed mods, resource packs or shader packs.",
        content::list,
    ),
    method(
        "content.search",
        "Searches Modrinth for content that fits the instance.",
        content::search,
    ),
    method(
        "content.versions",
        "A Modrinth project's versions, with whether each fits.",
        content::versions,
    ),
    method(
        "content.install",
        "Installs a Modrinth project (with its required mods).",
        content::install,
    ),
    method(
        "content.add_file",
        "Adds a local mod, resource pack or shader pack file.",
        content::add_file,
    ),
    destructive(
        "content.remove",
        "Moves installed content to the Trash.",
        content::remove,
    ),
    method(
        "content.enable",
        "Turns installed content on.",
        content::enable,
    ),
    method(
        "content.disable",
        "Turns installed content off.",
        content::disable,
    ),
    method(
        "content.updates",
        "Newer versions of installed content.",
        content::updates,
    ),
    method(
        "content.update",
        "Updates installed content (some or all).",
        content::update,
    ),
    method(
        "content.switch_loader",
        "Changes an instance's mod loader.",
        content::switch_loader,
    ),
    // modpacks
    method(
        "modpacks.search",
        "Searches Modrinth for modpacks.",
        content::modpack_search,
    ),
    method(
        "modpacks.install",
        "Makes an instance from a modpack (Modrinth or .mrpack).",
        content::modpack_install,
    ),
    method(
        "modpacks.check",
        "Whether an instance's modpack has a newer version.",
        content::modpack_check,
    ),
    method(
        "modpacks.update",
        "Updates an instance's modpack (shows the plan first).",
        content::modpack_update,
    ),
    // snapshots
    method(
        "snapshots.list",
        "An instance's snapshots, newest first: name, date, sizes (total and new), parts.",
        snapshots::list,
    ),
    method(
        "snapshots.create",
        "Takes a snapshot of an instance (include: part ids; default all but screenshots and other; worlds: folder names).",
        snapshots::create,
    ),
    destructive(
        "snapshots.revert",
        "Puts back the chosen parts of a snapshot (default: all it has), after an automatic snapshot of them.",
        snapshots::revert,
    ),
    method(
        "snapshots.export",
        "Writes a snapshot to a .lodesnap file (absolute path; a folder names it).",
        snapshots::export,
    ),
    method(
        "snapshots.import",
        "Makes a new instance from a .lodesnap file (absolute path); preview: true only reads it. With mods, needs confirm.",
        snapshots::import,
    ),
    destructive(
        "snapshots.delete",
        "Deletes a snapshot and the files no other snapshot uses.",
        snapshots::delete,
    ),
    // worlds
    method("worlds.list", "An instance's worlds.", worlds::list),
    method("worlds.backups", "A world's backups.", worlds::backups),
    method("worlds.backup", "Backs up a world.", worlds::backup),
    method(
        "worlds.restore",
        "Restores a backup as a copy, or replaces the world with it.",
        worlds::restore,
    ),
    destructive(
        "worlds.delete",
        "Moves a world to the Trash.",
        worlds::delete,
    ),
    destructive(
        "worlds.delete_backup",
        "Moves a backup to the Trash.",
        worlds::delete_backup,
    ),
    method(
        "worlds.download",
        "Downloads a server's world from its core into an instance.",
        servers::download_world,
    ),
    // accounts
    method(
        "accounts.list",
        "Accounts and the active one. Never tokens.",
        accounts::list,
    ),
    method(
        "accounts.switch",
        "Makes an account the active one.",
        accounts::switch,
    ),
    method(
        "accounts.add_offline",
        "Adds an offline account (development builds or LODESTONE_LAUNCHER_OFFLINE=1).",
        accounts::add_offline,
    ),
    destructive(
        "accounts.remove",
        "Removes an account and its saved sign-in.",
        accounts::remove,
    ),
    method(
        "accounts.sign_in",
        "Starts a Microsoft sign-in with a device code; returns the code and URL.",
        accounts::sign_in,
    ),
    // import
    method(
        "import.detect",
        "Other launchers' instances on this Mac, or in a folder.",
        import::detect,
    ),
    method(
        "import.run",
        "Imports instances from another launcher.",
        import::run,
    ),
    // cores and servers
    method(
        "cores.list",
        "Lodestone cores and their servers.",
        servers::cores_list,
    ),
    method(
        "cores.add",
        "Adds a core (and signs in to it).",
        servers::cores_add,
    ),
    destructive(
        "cores.remove",
        "Removes a core and forgets its key on this device.",
        servers::cores_remove,
    ),
    destructive(
        "cores.trust",
        "Trusts a core for mods: its mods install without the advisory.",
        servers::cores_trust,
    ),
    method(
        "cores.untrust",
        "Stops trusting a core for mods.",
        servers::cores_untrust,
    ),
    method(
        "cores.trust_certificate",
        "Pins a core's certificate after the user compared its fingerprint.",
        servers::cores_trust_certificate,
    ),
    method(
        "cores.sign_out",
        "Forgets this device's key for a core.",
        servers::cores_sign_out,
    ),
    method(
        "servers.list",
        "Servers on the cores.",
        servers::servers_list,
    ),
    method("servers.start", "Starts a server.", servers::start),
    method("servers.stop", "Stops a server.", servers::stop),
    method(
        "servers.join",
        "Plays the server's linked instance and joins it.",
        servers::join,
    ),
    method(
        "servers.create_instance",
        "Makes an instance matching a server, with its mods.",
        servers::create_instance,
    ),
    method(
        "servers.link",
        "Links an instance to a server.",
        servers::link,
    ),
    method(
        "servers.unlink",
        "Unlinks an instance from its server.",
        servers::unlink,
    ),
    method(
        "servers.host",
        "Hosts an instance's modpack as a new server on a core.",
        servers::host,
    ),
    method(
        "servers.resource_pack",
        "Saves a server's resource pack into an instance.",
        servers::resource_pack,
    ),
    method(
        "prompts.list",
        "Questions the window is asking about servers.",
        servers::prompts_list,
    ),
    method(
        "prompts.answer",
        "Answers a question the window is asking.",
        servers::prompts_answer,
    ),
    // jobs and events
    method(
        "jobs.list",
        "Long operations, running and finished.",
        jobs::list,
    ),
    method("jobs.get", "One job.", jobs::get),
    method(
        "jobs.wait",
        "Waits for a job, with progress lines.",
        jobs::wait,
    ),
    method("jobs.cancel", "Cancels a job.", jobs::cancel),
    method(
        "events.subscribe",
        "Streams log lines, game states, progress, jobs and notices.",
        events::subscribe,
    ),
    method(
        "events.unsubscribe",
        "Ends a subscription.",
        events::unsubscribe,
    ),
    // debug traces
    method(
        "trace.topics",
        "The debug trace topics, what each says, and how many events each had.",
        trace::topics,
    ),
    method(
        "trace.subscribe",
        "Streams debug trace events for topic patterns, after the last `backlog` earlier ones.",
        trace::subscribe,
    ),
    method(
        "trace.unsubscribe",
        "Ends a trace subscription.",
        trace::unsubscribe,
    ),
    method(
        "trace.snapshot",
        "What the window shows now (page, tab, instance, dialog, size, zoom), games and running jobs.",
        trace::snapshot,
    ),
    // perf
    method(
        "perf.show",
        "The launcher's memory, CPU, threads and open files, frame rate and frame times, a running game's memory and CPU, decoded images, the content cache, jobs, downloads, CLI clients and the trace bus. Takes a second.",
        perf::show,
    ),
    method(
        "perf.watch",
        "The same as perf.show every second, as perf events. End it with events.unsubscribe.",
        perf::watch,
    ),
];

/// A request being handled.
pub struct Call {
    pub control: Rc<Control>,
    pub id: Value,
    pub params: Value,
    pub outbox: Outbox,
    pub connection: u64,
}

impl Call {
    /// The parameters, read into `T`.
    pub fn params<T: DeserializeOwned>(&self) -> Result<T, ErrorObject> {
        serde_json::from_value(self.params.clone())
            .map_err(|err| api_error(codes::INVALID_PARAMS, format!("Bad parameters: {err}")))
    }

    /// Refuses a destructive request without `"confirm": true`.
    pub fn confirm(&self, what: impl Into<String>) -> Result<(), ErrorObject> {
        if confirmed(&self.params) {
            return Ok(());
        }
        let what = what.into();
        Err(api_error(
            codes::CONFIRMATION_REQUIRED,
            format!("This {what}. Ask the user first, then pass --yes (\"confirm\": true)."),
        ))
    }

    /// `"wait": true`: answer when the job ends, with progress lines until then.
    pub fn wait(&self) -> bool {
        self.params.get("wait") == Some(&Value::Bool(true))
    }
}

/// `"confirm": true`, and nothing else, confirms.
pub fn confirmed(params: &Value) -> bool {
    params.get("confirm") == Some(&Value::Bool(true))
}

/// The window's views the API keeps in step.
pub struct Views {
    pub content: Option<Entity<ContentView>>,
    pub worlds: Option<Entity<WorldsView>>,
}

/// A ready reply.
pub fn ready(result: ApiResult) -> Reply {
    Task::ready(result)
}

/// The control state, on the main thread.
pub struct Control {
    pub accounts: Entity<Accounts>,
    pub instances: Entity<Instances>,
    pub servers: Entity<Servers>,
    /// The Content and Worlds tabs, reloaded after a change to the instance they show.
    pub content: Option<Entity<ContentView>>,
    pub worlds: Option<Entity<WorldsView>>,
    pub jobs: RefCell<Jobs>,
    pub notices: RefCell<events::Notices>,
    /// Running subscriptions by connection, then by number.
    subscriptions: RefCell<HashMap<u64, HashMap<u64, Task<()>>>>,
    /// Trace subscriptions by connection: (number, the bus's id).
    trace_subscriptions: RefCell<HashMap<u64, Vec<(u64, u64)>>>,
    next_subscription: RefCell<u64>,
    /// Who is connected, by connection.
    clients: RefCell<BTreeMap<u64, ClientInfo>>,
    socket: PathBuf,
    started: Instant,
    _subscriptions: Vec<Subscription>,
}

impl Control {
    fn new(
        accounts: Entity<Accounts>,
        instances: Entity<Instances>,
        servers: Entity<Servers>,
        views: Views,
        socket: PathBuf,
        cx: &mut App,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &std::rc::Weak<Control>| {
            let on_instances = {
                let weak = weak.clone();
                cx.subscribe(&instances, move |_, event, _| {
                    if let Some(control) = weak.upgrade() {
                        control.notices.borrow_mut().instances_event(event);
                    }
                })
            };
            let on_servers = {
                let weak = weak.clone();
                cx.subscribe(&servers, move |_, event, _| {
                    if let Some(control) = weak.upgrade() {
                        control.notices.borrow_mut().servers_event(event);
                    }
                })
            };
            Control {
                accounts,
                instances,
                servers,
                content: views.content,
                worlds: views.worlds,
                jobs: RefCell::new(Jobs::default()),
                notices: RefCell::new(events::Notices::default()),
                subscriptions: RefCell::new(HashMap::new()),
                trace_subscriptions: RefCell::new(HashMap::new()),
                next_subscription: RefCell::new(1),
                clients: RefCell::new(BTreeMap::new()),
                socket,
                started: Instant::now(),
                _subscriptions: vec![on_instances, on_servers],
            }
        })
    }

    fn handle(self: &Rc<Self>, connection: u64, request: Request, outbox: Outbox, cx: &mut App) {
        let id = request.id.clone();
        let Some(method) = METHODS.iter().find(|m| m.name == request.method) else {
            outbox.respond(
                id,
                Err(api_error(
                    codes::METHOD_NOT_FOUND,
                    format!(
                        "No method \"{}\". `meta.methods` lists them.",
                        request.method
                    ),
                )),
            );
            return;
        };
        let cli = self.instances.read(cx).settings().cli;
        if let Some(refused) = refusal(method.name, &cli) {
            outbox.respond(id, Err(refused));
            return;
        }
        let call = Call {
            control: self.clone(),
            id: id.clone(),
            params: request.params,
            outbox: outbox.clone(),
            connection,
        };
        let reply = (method.handler)(call, cx);
        cx.spawn(async move |_| {
            let result = reply.await;
            outbox.respond(id, result);
        })
        .detach();
    }

    fn closed(&self, connection: u64) {
        self.clients.borrow_mut().remove(&connection);
        self.subscriptions.borrow_mut().remove(&connection);
        for (_, id) in self
            .trace_subscriptions
            .borrow_mut()
            .remove(&connection)
            .unwrap_or_default()
        {
            launcher_core::trace::bus().unsubscribe(id);
        }
    }

    /// The connected clients, with their subscriptions counted.
    fn client_list(&self) -> Vec<ClientInfo> {
        let events = self.subscriptions.borrow();
        let traces = self.trace_subscriptions.borrow();
        self.clients
            .borrow()
            .values()
            .map(|client| ClientInfo {
                subscriptions: events.get(&client.connection).map_or(0, |s| s.len())
                    + traces.get(&client.connection).map_or(0, |s| s.len()),
                ..client.clone()
            })
            .collect()
    }

    pub(crate) fn insert_trace_subscription(&self, connection: u64, number: u64, id: u64) {
        self.trace_subscriptions
            .borrow_mut()
            .entry(connection)
            .or_default()
            .push((number, id));
    }

    pub(crate) fn remove_trace_subscription(&self, connection: u64, number: u64) -> bool {
        let mut all = self.trace_subscriptions.borrow_mut();
        let Some(subs) = all.get_mut(&connection) else {
            return false;
        };
        let Some(index) = subs.iter().position(|(n, _)| *n == number) else {
            return false;
        };
        let (_, id) = subs.remove(index);
        launcher_core::trace::bus().unsubscribe(id)
    }

    /// The instance named by `reference`: its id, or its name in any case.
    pub fn instance(&self, reference: &str, cx: &App) -> Result<Instance, ErrorObject> {
        find_instance(self.instances.read(cx).list(), reference)
    }

    /// The instance named by the `instance` parameter.
    pub fn instance_param(&self, call: &Call, cx: &App) -> Result<Instance, ErrorObject> {
        let reference = call
            .params
            .get("instance")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                api_error(
                    codes::INVALID_PARAMS,
                    "Name the instance: \"instance\" is its id or its name.",
                )
            })?;
        self.instance(reference, cx)
    }

    /// Fails with `busy` while the instance's game prepares or runs.
    pub fn idle(&self, instance: &Instance, cx: &App) -> Result<(), ErrorObject> {
        if self.instances.read(cx).is_active(&instance.id) {
            return Err(api_error(
                codes::BUSY,
                format!(
                    "{} is starting or running. Stop it first (`play stop`).",
                    instance.name
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn reserve_subscription(&self) -> u64 {
        let mut next = self.next_subscription.borrow_mut();
        let number = *next;
        *next += 1;
        number
    }

    pub(crate) fn insert_subscription(&self, connection: u64, number: u64, task: Task<()>) {
        self.subscriptions
            .borrow_mut()
            .entry(connection)
            .or_default()
            .insert(number, task);
    }

    pub(crate) fn remove_subscription(&self, connection: u64, number: u64) -> bool {
        self.subscriptions
            .borrow_mut()
            .get_mut(&connection)
            .is_some_and(|subs| subs.remove(&number).is_some())
    }
}

/// The instance named by `reference` in `list`: its id, or its name in any case.
pub fn find_instance(list: &[Instance], reference: &str) -> Result<Instance, ErrorObject> {
    if let Some(instance) = list.iter().find(|i| i.id == reference) {
        return Ok(instance.clone());
    }
    let matches: Vec<&Instance> = list
        .iter()
        .filter(|i| i.name.eq_ignore_ascii_case(reference.trim()))
        .collect();
    match matches.as_slice() {
        [instance] => Ok((*instance).clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("No instance is called \"{reference}\". List them with `instances list`."),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("More than one instance is called \"{reference}\"; use its id."),
        )
        .with_data(json!({
            "matches": matches.iter().map(|i| json!({ "id": i.id, "name": i.name })).collect::<Vec<_>>()
        }))),
    }
}

/// Waits until `check` returns something, checking every `every`. Gives up with
/// `timeout` after `limit`, or when the client went away.
pub async fn poll_until<T>(
    cx: &mut AsyncApp,
    outbox: Option<&Outbox>,
    every: Duration,
    limit: Duration,
    mut check: impl FnMut(&mut App) -> Option<T>,
) -> Result<T, ErrorObject> {
    let started = Instant::now();
    loop {
        if let Some(value) = cx.update(&mut check) {
            return Ok(value);
        }
        if outbox.is_some_and(Outbox::is_closed) {
            return Err(api_error(codes::CANCELLED, "The client went away."));
        }
        if started.elapsed() > limit {
            return Err(api_error(
                codes::TIMEOUT,
                "The launcher stopped waiting; whatever it was doing goes on.",
            ));
        }
        cx.background_executor().timer(every).await;
    }
}

fn meta_ping(call: Call, cx: &mut App) -> Reply {
    let cli = call.control.instances.read(cx).settings().cli;
    ready(Ok(json!({
        "access": access_json(&cli),
        "app": crate::APP_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
        "pid": std::process::id(),
        "socket": call.control.socket,
        "uptime_secs": call.control.started.elapsed().as_secs(),
    })))
}

fn meta_methods(_: Call, _: &mut App) -> Reply {
    ready(Ok(json!({
        "methods": METHODS
            .iter()
            .map(|m| json!({
                "name": m.name,
                "summary": m.summary,
                "destructive": m.destructive,
                "read_only": reads_only(m.name),
            }))
            .collect::<Vec<_>>(),
        "error_codes": codes::ALL
            .iter()
            .map(|(code, exit, meaning)| json!({ "code": code, "exit": exit, "meaning": meaning }))
            .collect::<Vec<_>>(),
    })))
}

fn meta_status(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let instances = instances::summaries(control, cx);
    let accounts = accounts::summary(control, cx);
    let jobs = control.jobs.borrow().to_json(true);
    let cores = servers::cores_summary(control, cx);
    let cli = control.instances.read(cx).settings().cli;
    ready(Ok(json!({
        "access": access_json(&cli),
        "instances": instances,
        "accounts": accounts,
        "jobs": jobs,
        "cores": cores,
    })))
}

/// Hands each request to the main thread.
struct Forward {
    tx: futures::channel::mpsc::UnboundedSender<Incoming>,
    /// Which time the socket was opened: its connection numbers start at 1 again, so
    /// the main thread keys connections by both.
    generation: u64,
}

impl Forward {
    fn key(&self, connection: u64) -> u64 {
        (self.generation << 32) | (connection & 0xffff_ffff)
    }
}

enum Incoming {
    Connected(u64),
    Request {
        connection: u64,
        request: Box<Request>,
        outbox: Outbox,
    },
    Closed(u64),
}

impl Backend for Forward {
    fn connected(&self, connection: u64) {
        let _ = self
            .tx
            .unbounded_send(Incoming::Connected(self.key(connection)));
    }

    fn request(&self, connection: u64, request: Request, outbox: Outbox) {
        let id = request.id.clone();
        let sent = self.tx.unbounded_send(Incoming::Request {
            connection: self.key(connection),
            request: Box::new(request),
            outbox: outbox.clone(),
        });
        if sent.is_err() {
            outbox.respond(
                id,
                Err(api_error(codes::UNAVAILABLE, "The launcher is quitting.")),
            );
        }
    }

    fn disconnected(&self, connection: u64) {
        let _ = self
            .tx
            .unbounded_send(Incoming::Closed(self.key(connection)));
    }
}

/// A program connected to the socket, for Settings > Developer.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientInfo {
    pub connection: u64,
    /// What it said it is in `meta.hello`, such as "lodestone-launcher-cli 0.4.0".
    pub name: Option<String>,
    pub pid: Option<u32>,
    pub since: std::time::SystemTime,
    /// Event and trace subscriptions open on it.
    pub subscriptions: usize,
}

/// The socket's state, for Settings > Developer. Notified when it opens or closes, and
/// when a client comes, goes, says hello or subscribes.
#[derive(Clone, Debug, Default)]
pub struct ControlStatus {
    pub socket: PathBuf,
    pub running: bool,
    /// Why it couldn't open, when it is meant to be open.
    pub error: Option<String>,
    pub clients: Vec<ClientInfo>,
}

/// The control socket, kept in step with Settings > Developer > Command line.
struct Service {
    control: Rc<Control>,
    tx: futures::channel::mpsc::UnboundedSender<Incoming>,
    handle: Option<ServerHandle>,
    generation: u64,
    status: Entity<ControlStatus>,
    _incoming: Task<()>,
}

impl gpui::Global for Service {}

/// Game output may go to clients: `play.logs`, `log` events and `game.log` traces.
static GAME_LOGS: AtomicBool = AtomicBool::new(true);

pub(crate) fn game_logs_allowed() -> bool {
    GAME_LOGS.load(Ordering::Relaxed)
}

/// Serves the control socket for the app's entities, when Settings > Developer
/// allows it. Without a socket (another launcher already serves this data
/// directory, or the folder can't be used) the app runs as before, and the
/// Developer page says why.
pub fn start(shell: &LauncherShell, cx: &mut gpui::Context<LauncherShell>) {
    start_service(shell, cx);
    // the Developer page shows the socket and its clients
    if let Some(status) = status(cx) {
        // only while it is on screen: a client coming or going redraws nothing else
        cx.observe(&status, |shell, _, cx| {
            if shell.shows_developer_settings() {
                cx.notify();
            }
        })
        .detach();
    }
}

fn start_service(shell: &LauncherShell, cx: &mut App) {
    let accounts = shell.accounts();
    let instances = shell.instances();
    let servers = shell.servers();
    let root = accounts.read(cx).data_dir().root().to_path_buf();
    let path = paths::socket_path(&root);
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Incoming>();
    let control = Control::new(
        accounts.clone(),
        instances.clone(),
        servers.clone(),
        Views {
            content: Some(shell.content().clone()),
            worlds: Some(shell.worlds().clone()),
        },
        path.clone(),
        cx,
    );
    let status = cx.new(|_| ControlStatus {
        socket: path,
        ..ControlStatus::default()
    });
    let incoming = cx.spawn({
        let control = control.clone();
        async move |cx| {
            while let Some(incoming) = rx.next().await {
                let control = control.clone();
                cx.update(|cx| match incoming {
                    Incoming::Connected(connection) => {
                        control.clients.borrow_mut().insert(
                            connection,
                            ClientInfo {
                                connection,
                                name: None,
                                pid: None,
                                since: std::time::SystemTime::now(),
                                subscriptions: 0,
                            },
                        );
                        refresh_status(cx);
                    }
                    Incoming::Request {
                        connection,
                        request,
                        outbox,
                    } => {
                        let changes_clients = CHANGES_CLIENTS.contains(&request.method.as_str());
                        control.handle(connection, *request, outbox, cx);
                        if changes_clients {
                            refresh_status(cx);
                        }
                    }
                    Incoming::Closed(connection) => {
                        control.closed(connection);
                        refresh_status(cx);
                    }
                });
            }
        }
    });
    cx.set_global(Service {
        control,
        tx,
        handle: None,
        generation: 0,
        status,
        _incoming: incoming,
    });
    cx.on_app_quit(|cx| {
        // removes the socket file
        if cx.has_global::<Service>() {
            cx.global_mut::<Service>().handle.take();
        }
        async {}
    })
    .detach();
    apply(cx);
}

/// Methods after which the Developer page's list of clients may change.
const CHANGES_CLIENTS: &[&str] = &[
    "meta.hello",
    "events.subscribe",
    "events.unsubscribe",
    "trace.subscribe",
    "trace.unsubscribe",
    "perf.watch",
];

/// Opens or closes the socket, and turns traces and game logs on or off, as the
/// settings say now. Closing it drops every client.
pub fn apply(cx: &mut App) {
    if !cx.has_global::<Service>() {
        return;
    }
    let cli = cx
        .global::<Service>()
        .control
        .instances
        .read(cx)
        .settings()
        .cli;
    let bus = launcher_core::trace::bus();
    if bus.is_enabled() != cli.traces {
        bus.set_enabled(cli.traces);
        if !cli.traces {
            // the bus ended them
            cx.global::<Service>()
                .control
                .trace_subscriptions
                .borrow_mut()
                .clear();
        }
    }
    GAME_LOGS.store(cli.game_logs, Ordering::Relaxed);
    let service = cx.global_mut::<Service>();
    let mut error = None;
    match (cli.enabled, service.handle.is_some()) {
        (true, false) => {
            service.generation += 1;
            let forward = Forward {
                tx: service.tx.clone(),
                generation: service.generation,
            };
            let socket = service.control.socket.clone();
            let served = Server::bind(&socket)
                .map_err(|err| err.to_string())
                .and_then(|server| {
                    server
                        .spawn(Arc::new(forward))
                        .map_err(|err| err.to_string())
                });
            match served {
                Ok(handle) => service.handle = Some(handle),
                Err(err) => {
                    eprintln!(
                        "lodestone-launcher: no control socket, so lodestone-launcher-cli can't reach this launcher: {err}"
                    );
                    error = Some(err);
                }
            }
        }
        (false, true) => {
            // closes every connection and removes the socket file; each client's
            // `Closed` ends its subscriptions
            service.handle.take();
        }
        _ => {}
    }
    let running = service.handle.is_some();
    let status = service.status.clone();
    status.update(cx, |status, cx| {
        status.running = running;
        if running || !cli.enabled {
            status.error = None;
        } else if error.is_some() {
            status.error = error;
        }
        cx.notify();
    });
    refresh_status(cx);
}

/// Copies the clients into the status, and redraws who shows it.
fn refresh_status(cx: &mut App) {
    let Some(service) = cx.try_global::<Service>() else {
        return;
    };
    let clients = service.control.client_list();
    let status = service.status.clone();
    status.update(cx, |status, cx| {
        if status.clients != clients {
            status.clients = clients;
            cx.notify();
        }
    });
}

/// The socket's state, to show and observe; `None` before [`start`].
pub fn status(cx: &App) -> Option<Entity<ControlStatus>> {
    cx.try_global::<Service>()
        .map(|service| service.status.clone())
        .or_else(|| {
            cx.try_global::<PreviewStatus>()
                .map(|preview| preview.0.clone())
        })
}

/// A made-up socket state for the offscreen screenshots, which serve no socket.
struct PreviewStatus(Entity<ControlStatus>);

impl gpui::Global for PreviewStatus {}

/// Shows `status` on the Developer page, for the offscreen screenshots.
pub fn preview(status: ControlStatus, cx: &mut App) {
    let status = cx.new(|_| status);
    cx.set_global(PreviewStatus(status));
}

/// Control API jobs running, for the performance monitor.
pub fn running_jobs(cx: &App) -> usize {
    cx.try_global::<Service>().map_or(0, |service| {
        service
            .control
            .jobs
            .borrow()
            .states()
            .iter()
            .filter(|(_, state)| *state == JobState::Running)
            .count()
    })
}

/// Programs connected to the socket; `None` while it is closed.
pub fn client_count(cx: &App) -> Option<usize> {
    let service = cx.try_global::<Service>()?;
    service
        .handle
        .is_some()
        .then(|| service.control.clients.borrow().len())
}

/// Closes one client's connection (the Developer page's Disconnect).
pub fn disconnect(connection: u64, cx: &mut App) {
    let Some(service) = cx.try_global::<Service>() else {
        return;
    };
    if connection >> 32 == service.generation
        && let Some(handle) = &service.handle
    {
        handle.disconnect(connection & 0xffff_ffff);
    }
}

/// Why `method` is refused under `cli`, if it is: a method that changes something
/// while access is read-only, a trace method while traces are off, or game output
/// while game logs are off.
pub fn refusal(method: &str, cli: &CliSettings) -> Option<ErrorObject> {
    if method.starts_with("trace.") && !cli.traces {
        return Some(api_error(
            codes::PERMISSION_DENIED,
            "Debug traces are off in Settings → Developer.",
        ));
    }
    if GAME_LOG_METHODS.contains(&method) && !cli.game_logs {
        return Some(api_error(
            codes::PERMISSION_DENIED,
            "Game logs over the command line are off in Settings → Developer.",
        ));
    }
    if cli.access == CliAccess::ReadOnly && !reads_only(method) {
        return Some(
            api_error(
                codes::PERMISSION_DENIED,
                format!(
                    "Read-only CLI access is on in Settings → Developer, and {method} changes things."
                ),
            )
            .with_data(json!({ "access": "read_only", "method": method })),
        );
    }
    None
}

/// Methods that send game output.
const GAME_LOG_METHODS: &[&str] = &["play.logs"];

/// Whether `method` only reads. Anything not listed here changes something, so a
/// new method is refused under read-only access until it is listed.
pub fn reads_only(method: &str) -> bool {
    READ_METHODS.contains(&method)
}

/// The methods read-only access allows: they change nothing in the launcher, its
/// data folder or elsewhere, and write no file.
pub const READ_METHODS: &[&str] = &[
    "meta.ping",
    "meta.methods",
    "meta.status",
    "meta.hello",
    "instances.list",
    "instances.show",
    "instances.versions",
    "instances.loader_versions",
    "instances.share_code",
    "play.status",
    "play.logs",
    "settings.instance.get",
    "settings.defaults.get",
    "settings.general.get",
    "settings.developer.get",
    "skins.list",
    "skins.check",
    "storage.show",
    "shared.get",
    "shared.instance.get",
    "content.list",
    "content.search",
    "content.versions",
    "content.updates",
    "modpacks.search",
    "modpacks.check",
    "snapshots.list",
    "worlds.list",
    "worlds.backups",
    "accounts.list",
    "import.detect",
    "cores.list",
    "servers.list",
    "prompts.list",
    "jobs.list",
    "jobs.get",
    "jobs.wait",
    "events.subscribe",
    "events.unsubscribe",
    "trace.topics",
    "trace.subscribe",
    "trace.unsubscribe",
    "trace.snapshot",
    "perf.show",
    "perf.watch",
];

/// The access settings as `meta.ping` and `meta.status` report them.
fn access_json(cli: &CliSettings) -> Value {
    json!({
        "level": cli.access.as_str(),
        "traces": cli.traces,
        "game_logs": cli.game_logs,
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HelloParams {
    /// Such as "lodestone-launcher-cli 0.4.0".
    client: String,
    #[serde(default)]
    pid: Option<u32>,
}

/// A client says what it is, for the Developer page's list.
fn meta_hello(call: Call, cx: &mut App) -> Reply {
    let params: HelloParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let name: String = params
        .client
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if let Some(client) = call.control.clients.borrow_mut().get_mut(&call.connection) {
        client.name = Some(name);
        client.pid = params.pid;
    }
    let cli = call.control.instances.read(cx).settings().cli;
    ready(Ok(json!({
        "app": crate::APP_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
        "access": access_json(&cli),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_are_unique_and_dotted() {
        let mut seen = std::collections::HashSet::new();
        for method in METHODS {
            assert!(seen.insert(method.name), "{} twice", method.name);
            assert!(method.name.contains('.'), "{}", method.name);
            assert!(!method.summary.is_empty());
        }
    }

    #[test]
    fn instances_are_found_by_id_or_name() {
        let list = vec![
            Instance {
                id: "a1".into(),
                name: "Survival".into(),
                ..Instance::default()
            },
            Instance {
                id: "b2".into(),
                name: "Creative".into(),
                ..Instance::default()
            },
            Instance {
                id: "c3".into(),
                name: "creative".into(),
                ..Instance::default()
            },
        ];
        assert_eq!(find_instance(&list, "a1").unwrap().name, "Survival");
        assert_eq!(find_instance(&list, "survival").unwrap().id, "a1");
        assert_eq!(
            find_instance(&list, "nope").unwrap_err().code,
            codes::NOT_FOUND
        );
        let ambiguous = find_instance(&list, "CREATIVE").unwrap_err();
        assert_eq!(ambiguous.code, codes::AMBIGUOUS);
        assert_eq!(
            ambiguous.data.unwrap()["matches"].as_array().unwrap().len(),
            2
        );
        assert_eq!(find_instance(&list, "c3").unwrap().name, "creative");
    }

    #[test]
    fn destructive_calls_need_confirm() {
        assert!(!confirmed(&json!({})));
        assert!(!confirmed(&json!({ "confirm": "yes" })));
        assert!(!confirmed(&json!({ "confirm": 1 })));
        assert!(confirmed(&json!({ "confirm": true })));
        let destructive: Vec<&str> = METHODS
            .iter()
            .filter(|m| m.destructive)
            .map(|m| m.name)
            .collect();
        for name in [
            "instances.delete",
            "worlds.delete",
            "accounts.remove",
            "cores.remove",
            "shared.clear",
            "snapshots.revert",
            "snapshots.delete",
        ] {
            assert!(destructive.contains(&name), "{name} must need confirm");
        }
    }

    /// Every method that changes something, files written included. With
    /// [`READ_METHODS`] it must name every method, once.
    const WRITE_METHODS: &[&str] = &[
        "instances.select",
        "instances.create",
        "instances.rename",
        "instances.set_group",
        "instances.set_prerelease_updates",
        "instances.set_icon",
        "instances.set_background",
        "instances.duplicate",
        "instances.delete",
        "instances.export",
        "instances.import_code",
        "instances.verify",
        "play.launch",
        "play.stop",
        "play.clear_logs",
        "settings.instance.set",
        "settings.instance.reset",
        "settings.defaults.set",
        "settings.defaults.reset",
        "settings.general.set",
        "settings.developer.set",
        "skins.use",
        "skins.import",
        "skins.export",
        "skins.fix",
        "storage.clean",
        "shared.set_value",
        "shared.set_category",
        "shared.sync_from",
        "shared.copy_from",
        "shared.clear",
        "shared.instance.set",
        "content.install",
        "content.add_file",
        "content.remove",
        "content.enable",
        "content.disable",
        "content.update",
        "content.switch_loader",
        "modpacks.install",
        "modpacks.update",
        "snapshots.create",
        "snapshots.revert",
        "snapshots.export",
        "snapshots.import",
        "snapshots.delete",
        "worlds.backup",
        "worlds.restore",
        "worlds.delete",
        "worlds.delete_backup",
        "worlds.download",
        "accounts.switch",
        "accounts.add_offline",
        "accounts.remove",
        "accounts.sign_in",
        "import.run",
        "cores.add",
        "cores.remove",
        "cores.trust",
        "cores.untrust",
        "cores.trust_certificate",
        "cores.sign_out",
        "servers.start",
        "servers.stop",
        "servers.join",
        "servers.create_instance",
        "servers.link",
        "servers.unlink",
        "servers.host",
        "servers.resource_pack",
        "prompts.answer",
        "jobs.cancel",
    ];

    #[test]
    fn every_method_is_classified_as_read_or_write() {
        for method in METHODS {
            let read = READ_METHODS.contains(&method.name);
            let write = WRITE_METHODS.contains(&method.name);
            assert!(
                read != write,
                "{} must be in exactly one of READ_METHODS and WRITE_METHODS",
                method.name
            );
            assert_eq!(reads_only(method.name), read);
        }
        for name in READ_METHODS.iter().chain(WRITE_METHODS) {
            assert!(
                METHODS.iter().any(|m| m.name == *name),
                "{name} is listed but no such method"
            );
        }
        // nothing destructive is read-only
        for method in METHODS.iter().filter(|m| m.destructive) {
            assert!(!reads_only(method.name), "{}", method.name);
        }
        assert!(!reads_only("no.such_method"));
    }

    #[test]
    fn read_only_access_refuses_every_write() {
        let read_only = CliSettings {
            access: CliAccess::ReadOnly,
            ..CliSettings::default()
        };
        for method in METHODS {
            let refused = refusal(method.name, &read_only);
            if READ_METHODS.contains(&method.name) {
                assert!(refused.is_none(), "{} was refused", method.name);
            } else {
                let error = refused.unwrap_or_else(|| panic!("{} was allowed", method.name));
                assert_eq!(error.code, codes::PERMISSION_DENIED);
                assert!(error.message.contains("Read-only"), "{}", error.message);
            }
        }
        // full access refuses nothing
        for method in METHODS {
            assert!(refusal(method.name, &CliSettings::default()).is_none());
        }
    }

    #[test]
    fn traces_and_game_logs_can_be_refused() {
        let off = CliSettings {
            traces: false,
            game_logs: false,
            ..CliSettings::default()
        };
        for name in ["trace.topics", "trace.subscribe", "trace.snapshot"] {
            assert_eq!(refusal(name, &off).unwrap().code, codes::PERMISSION_DENIED);
        }
        assert_eq!(
            refusal("play.logs", &off).unwrap().code,
            codes::PERMISSION_DENIED
        );
        assert!(refusal("play.status", &off).is_none());
        assert!(refusal("meta.ping", &off).is_none());
    }

    #[test]
    fn no_method_changes_the_command_line_settings() {
        // they are changed in the window only: no method's parameters reach them
        for method in METHODS {
            assert!(
                !method.name.contains("cli") && !method.name.contains("control"),
                "{}",
                method.name
            );
        }
        // settings.developer.set takes only its own fields
        for params in [
            json!({ "sign_in_as_minecraft_launcher": false, "cli": { "access": "full" } }),
            json!({ "sign_in_as_minecraft_launcher": false, "access": "full" }),
            json!({ "sign_in_as_minecraft_launcher": false, "enabled": true }),
            json!({ "sign_in_as_minecraft_launcher": false, "traces": true }),
        ] {
            assert!(
                settings::developer_params(params.clone()).is_err(),
                "{params} was taken"
            );
        }
        assert!(
            settings::developer_params(json!({ "sign_in_as_minecraft_launcher": true })).is_ok()
        );
    }

    #[test]
    fn core_errors_map_to_codes() {
        assert_eq!(core_error(CoreError::SignedOut).code, codes::NOT_SIGNED_IN);
        assert_eq!(
            core_error(CoreError::Unreachable("x".into())).code,
            codes::UNAVAILABLE
        );
        let cert = core_error(CoreError::Untrusted {
            host: "core.local".into(),
            fingerprint: Some("AB:CD".into()),
        });
        assert_eq!(cert.code, codes::UNTRUSTED_CERTIFICATE);
        assert_eq!(cert.data.unwrap()["fingerprint"], "AB:CD");
        assert_eq!(
            core_error(CoreError::BadCredentials).code,
            codes::PERMISSION_DENIED
        );
    }
}
