//! The control API: what `lodestone-launcher-cli` (and other programs of this user)
//! can do with the running launcher, over a Unix socket in the data directory.
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
mod servers;
mod settings;
mod skins;
mod storage;
mod trace;
mod worlds;

use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::StreamExt as _;
use gpui::{App, AsyncApp, Entity, Subscription, Task};
use launcher_control::{
    ErrorObject, PROTOCOL_VERSION, Request, codes, paths,
    server::{Backend, Outbox, Server, ServerHandle},
};
use launcher_core::{cores::CoreError, instances::Instance};
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

fn meta_ping(call: Call, _: &mut App) -> Reply {
    ready(Ok(json!({
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
            .map(|m| json!({ "name": m.name, "summary": m.summary, "destructive": m.destructive }))
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
    ready(Ok(json!({
        "instances": instances,
        "accounts": accounts,
        "jobs": jobs,
        "cores": cores,
    })))
}

/// Hands each request to the main thread.
struct Forward {
    tx: futures::channel::mpsc::UnboundedSender<Incoming>,
}

enum Incoming {
    Request {
        connection: u64,
        request: Box<Request>,
        outbox: Outbox,
    },
    Closed(u64),
}

impl Backend for Forward {
    fn request(&self, connection: u64, request: Request, outbox: Outbox) {
        let id = request.id.clone();
        let sent = self.tx.unbounded_send(Incoming::Request {
            connection,
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
        let _ = self.tx.unbounded_send(Incoming::Closed(connection));
    }
}

/// Serves the control socket for the app's entities. Without a socket (another
/// launcher already serves this data directory, or the folder can't be used) the app
/// runs as before, and says so on stderr.
pub fn start(shell: &LauncherShell, cx: &mut App) {
    let accounts = shell.accounts();
    let instances = shell.instances();
    let servers = shell.servers();
    let root = accounts.read(cx).data_dir().root().to_path_buf();
    let path = paths::socket_path(&root);
    let server = match Server::bind(&path) {
        Ok(server) => server,
        Err(err) => {
            eprintln!(
                "lodestone-launcher: no control socket, so lodestone-launcher-cli can't reach this launcher: {err}"
            );
            return;
        }
    };
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Incoming>();
    let handle: ServerHandle = match server.spawn(Arc::new(Forward { tx })) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("lodestone-launcher: couldn't serve the control socket: {err}");
            return;
        }
    };
    let handle = Rc::new(RefCell::new(Some(handle)));
    cx.on_app_quit({
        let handle = handle.clone();
        move |_| {
            // removes the socket file
            handle.borrow_mut().take();
            async {}
        }
    })
    .detach();
    let control = Control::new(
        accounts.clone(),
        instances.clone(),
        servers.clone(),
        Views {
            content: Some(shell.content().clone()),
            worlds: Some(shell.worlds().clone()),
        },
        path,
        cx,
    );
    cx.spawn(async move |cx| {
        let _handle = handle;
        while let Some(incoming) = rx.next().await {
            match incoming {
                Incoming::Request {
                    connection,
                    request,
                    outbox,
                } => {
                    let control = control.clone();
                    cx.update(|cx| control.handle(connection, *request, outbox, cx));
                }
                Incoming::Closed(connection) => control.closed(connection),
            }
        }
    })
    .detach();
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
        ] {
            assert!(destructive.contains(&name), "{name} must need confirm");
        }
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
