//! What a server serves its players (`docs/instance-images.md`, "Server to player"): the
//! client view of its current content image, a delta to it from a view a player has, and
//! which objects a player may fetch.
//!
//! A view is a pure function of the image and the rules (`lodestone_image::view`): every
//! file's sides were settled when the image was recorded ([`super::env`]), so nothing is
//! looked up while answering. That makes everything here cacheable for good:
//!
//! - views by content id (a content id is its contents, and a view of it under one set of
//!   rules never changes);
//! - deltas by the two view ids;
//! - which content images each view id comes from, per instance, learned lazily from the
//!   instance's manifests the first time a player names a view the core hasn't seen.
//!
//! A request costs a look at the state file (its size, time and inode: the pointer,
//! `content-state.json`'s `current`, is read again only when they change) and then cache
//! hits. The pointer moves only after its manifest is saved, and manifests are
//! written by an atomic rename, so a request never sees a half-written image; the current
//! image and its parent are never pruned, and saving a manifest checks every object it
//! names is in the store, so the objects a current view names are there. Nothing here holds
//! one of the core's locks: a player's request never waits for a record.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use color_eyre::eyre::WrapErr;
use lodestone_image::store::{Store, StoreError};
use lodestone_image::{delta, view, Image, Kind, Side, Source, RULES};
use tracing::warn;

use crate::error::{Error, ErrorKind};
use crate::types::InstanceUuid;

use super::state::StateStamp;
use super::{blocking, ImageService, InstanceContext};

/// The most view ids a player may send in `have`.
pub const MAX_HAVE: usize = 16;
/// How many views are kept, by content id (every server's current view and a few older
/// ones players still have), and their JSON's bytes in all. A view takes a few times its
/// JSON in memory; the most recently used one is always kept, however large.
const VIEW_CACHE: usize = 32;
const VIEW_CACHE_BYTES: usize = 32 << 20;
/// How many deltas are kept, and their bytes in all.
const DELTA_CACHE: usize = 64;
const DELTA_CACHE_BYTES: usize = 16 << 20;
/// How long after a failed first record ([`ImageService::record_first`]) the next may
/// start; it doubles with each failure in a row, up to [`FIRST_RECORD_MAX_WAIT`]. Also how
/// long after a record that stores missing objects again ([`ImageService::record_again`])
/// the next may start, whatever its outcome.
pub const FIRST_RECORD_WAIT: Duration = Duration::from_secs(60);
pub const FIRST_RECORD_MAX_WAIT: Duration = Duration::from_secs(30 * 60);

/// The client view of one content image, ready to send.
pub struct ServedView {
    /// The content image it is the view of.
    pub content_id: String,
    /// The view, sealed: its id is what players name it by.
    pub view: Image,
    /// The view's manifest as sent.
    pub json: Bytes,
    /// What a player may fetch from this server while this view is current, by SHA-256
    /// with its size: entries with source `object`, and the client settings' stored files
    /// (icon, background).
    objects: HashMap<String, u64>,
}

impl ServedView {
    fn new(content_id: String, view: Image) -> ServedView {
        let mut objects = HashMap::new();
        for entry in &view.entries {
            if entry.source == (Source::Object {}) {
                objects.insert(entry.sha256.clone(), entry.size);
            }
        }
        if let Some(client) = &view.settings.client {
            for stored in client.icon.iter().chain(client.background.iter()) {
                objects.insert(stored.sha256.clone(), stored.size);
            }
        }
        // An image is maps with string keys and plain values: serializing can't fail.
        let json = Bytes::from(serde_json::to_vec(&view).unwrap_or_default());
        ServedView {
            content_id,
            view,
            json,
            objects,
        }
    }

    /// The view's id.
    pub fn id(&self) -> &str {
        &self.view.id
    }

    /// The size of the object `sha256` when this view lets a player fetch it.
    pub fn object_size(&self, sha256: &str) -> Option<u64> {
        self.objects.get(sha256).copied()
    }
}

/// The answer to a player that says which views it has.
pub enum ClientAnswer {
    /// The player has the current view.
    Current(Arc<ServedView>),
    /// The whole current view: the player has none the core knows (a player whose views
    /// were all pruned diffs it against the view it applied itself), or the delta would be
    /// no smaller.
    View(Arc<ServedView>),
    /// The changes from a view the player has to the current one, as sent.
    Delta { view: Arc<ServedView>, json: Bytes },
}

impl ClientAnswer {
    /// The current view.
    pub fn current(&self) -> &Arc<ServedView> {
        match self {
            ClientAnswer::Current(view) | ClientAnswer::View(view) => view,
            ClientAnswer::Delta { view, .. } => view,
        }
    }
}

/// A map that keeps at most `cap` values and `max_weight` of their weight, forgetting the
/// least recently used first. Small enough that a linear search for the oldest is fine.
struct Bounded<K, V> {
    clock: u64,
    weight: usize,
    entries: HashMap<K, (u64, V)>,
}

impl<K: Eq + Hash + Clone, V: Clone> Bounded<K, V> {
    fn new() -> Self {
        Bounded {
            clock: 0,
            weight: 0,
            entries: HashMap::new(),
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        self.clock += 1;
        let now = self.clock;
        self.entries.get_mut(key).map(|(used, value)| {
            *used = now;
            value.clone()
        })
    }

    fn insert(&mut self, key: K, value: V, cap: usize, max_weight: usize, weigh: fn(&V) -> usize) {
        self.clock += 1;
        if let Some((_, old)) = self.entries.insert(key, (self.clock, value.clone())) {
            self.weight -= weigh(&old);
        }
        self.weight += weigh(&value);
        while self.entries.len() > cap || (self.weight > max_weight && self.entries.len() > 1) {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((_, gone)) = self.entries.remove(&oldest) {
                self.weight -= weigh(&gone);
            }
        }
    }
}

/// Which content images each client view id comes from, for one instance.
#[derive(Default)]
struct Known {
    /// Client view id to the content images with that view: many share one, since a change
    /// only the server sees (`server.properties`, a server-only mod) leaves it as it was.
    views: HashMap<String, HashSet<String>>,
    /// Manifests already looked at, each with its client view id (`None`: not a content
    /// image, or unreadable), so each is read once.
    seen: HashMap<String, Option<String>>,
}

impl Known {
    fn learn(&mut self, manifest: &str, view_id: Option<&str>) {
        if let Some(view_id) = view_id {
            self.views
                .entry(view_id.to_string())
                .or_default()
                .insert(manifest.to_string());
        }
        self.seen
            .insert(manifest.to_string(), view_id.map(str::to_string));
    }

    /// Forgets a manifest that is gone. Its view stays known while another manifest has it.
    fn forget(&mut self, manifest: &str) {
        let Some(Some(view_id)) = self.seen.remove(manifest) else {
            return;
        };
        if let Some(contents) = self.views.get_mut(&view_id) {
            contents.remove(manifest);
            if contents.is_empty() {
                self.views.remove(&view_id);
            }
        }
    }
}

/// The caches of what players are served (see the module). Part of [`ImageService`].
pub(crate) struct ViewCache {
    views: StdMutex<Bounded<String, Arc<ServedView>>>,
    /// One computation of a content id's view at a time, so players who all ask at once
    /// for a new image wait for one.
    computing: StdMutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// `None`: the delta is no smaller than the view, which is sent instead.
    deltas: StdMutex<Bounded<(String, String), Option<Bytes>>>,
    known: StdMutex<HashMap<InstanceUuid, Known>>,
    /// One scan of an instance's manifests at a time.
    lookups: StdMutex<HashMap<InstanceUuid, Arc<tokio::sync::Mutex<()>>>>,
    /// Each instance's pointer as last read, with the state file's stamp then.
    pointers: StdMutex<HashMap<InstanceUuid, (StateStamp, Option<String>)>>,
    /// Instances a player's request is recording in the background, or whose next such
    /// record must wait.
    first_records: StdMutex<HashMap<InstanceUuid, FirstRecordState>>,
}

/// Where the background records ([`ImageService::record_first`],
/// [`ImageService::record_again`]) of an instance are.
enum FirstRecordState {
    Running,
    /// None may start before `retry_at`. `failures` is how many failed in a row.
    Waiting {
        failures: u32,
        retry_at: Instant,
    },
}

/// What [`ImageService::record_first`] (or [`ImageService::record_again`]) did.
#[derive(Debug, PartialEq, Eq)]
pub enum FirstRecord {
    /// The record is running (started by this request or an earlier one).
    Recording,
    /// The last record failed (or, for [`ImageService::record_again`], ran a moment ago);
    /// the next starts once this has passed.
    Failed { retry_after: Duration },
}

/// How long to wait after `failures` failed first records in a row.
fn first_record_wait(failures: u32) -> Duration {
    FIRST_RECORD_WAIT
        .saturating_mul(1 << failures.saturating_sub(1).min(16))
        .min(FIRST_RECORD_MAX_WAIT)
}

impl Default for ViewCache {
    fn default() -> ViewCache {
        ViewCache {
            views: StdMutex::new(Bounded::new()),
            computing: Default::default(),
            deltas: StdMutex::new(Bounded::new()),
            known: Default::default(),
            lookups: Default::default(),
            pointers: Default::default(),
            first_records: Default::default(),
        }
    }
}

fn locked<T>(mutex: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl ViewCache {
    /// Forgets a deleted instance. Its views and deltas age out.
    pub(crate) fn forget(&self, uuid: &InstanceUuid) {
        locked(&self.known).remove(uuid);
        locked(&self.lookups).remove(uuid);
        locked(&self.pointers).remove(uuid);
        locked(&self.first_records).remove(uuid);
    }

    /// The content images known to have the client view `view_id`.
    fn contents_of(&self, uuid: &InstanceUuid, view_id: &str) -> Vec<String> {
        locked(&self.known)
            .get(uuid)
            .and_then(|known| known.views.get(view_id))
            .map(|contents| contents.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn knows(&self, uuid: &InstanceUuid, view_id: &str) -> bool {
        locked(&self.known)
            .get(uuid)
            .is_some_and(|known| known.views.contains_key(view_id))
    }

    fn learn(&self, uuid: &InstanceUuid, view_id: &str, content_id: &str) {
        locked(&self.known)
            .entry(uuid.clone())
            .or_default()
            .learn(content_id, Some(view_id));
    }

    fn unlearn(&self, uuid: &InstanceUuid, content_id: &str) {
        if let Some(known) = locked(&self.known).get_mut(uuid) {
            known.forget(content_id);
        }
    }
}

/// The client view of the content image `content_id` of `uuid`, from its manifest (which
/// is checked against its id as it is read). Blocking.
fn served_view(store: &Store, uuid: &InstanceUuid, content_id: &str) -> Result<ServedView, Error> {
    // What a player sees is only the top message: the store's errors name its folders,
    // so they go only into the log, as causes.
    let image = store
        .load_manifest(uuid.as_ref(), content_id)
        .map_err(|e| match e {
            StoreError::Missing(_) => Error {
                kind: ErrorKind::NotFound,
                source: color_eyre::eyre::eyre!("The server's content image is gone"),
            },
            e => Error::from(
                color_eyre::Report::new(e)
                    .wrap_err(format!("Failed to read the content image {content_id}")),
            ),
        })?;
    let view = view(&image, Side::Client, RULES)
        .map_err(color_eyre::Report::new)
        .wrap_err(format!("Failed to compute the client view of {content_id}"))?;
    Ok(ServedView::new(content_id.to_string(), view))
}

impl ImageService {
    /// The client view of the instance's current content image, or `None` while it has
    /// none (start one with [`Self::record_first`]). A pointer whose manifest went missing
    /// or can't be read counts as none: the next record saves it again.
    pub async fn client_view(&self, uuid: &InstanceUuid) -> Result<Option<Arc<ServedView>>, Error> {
        if !Self::accepts(uuid) {
            return Err(Error {
                kind: ErrorKind::NotFound,
                source: color_eyre::eyre::eyre!("Server not found"),
            });
        }
        let Some(current) = self.current_pointer(uuid).await? else {
            return Ok(None);
        };
        match self.served(uuid, &current).await {
            Ok(view) => Ok(Some(view)),
            Err(e) => {
                if !matches!(e.kind, ErrorKind::NotFound) {
                    warn!(
                        "The current content image of {uuid} can't be read; recording it again: {:#}",
                        e.source
                    );
                }
                Ok(None)
            }
        }
    }

    /// The instance's current content image id, read from its state file only when the
    /// file changed since the last read ([`super::state::stamp`]).
    async fn current_pointer(&self, uuid: &InstanceUuid) -> Result<Option<String>, Error> {
        let cached = locked(&self.views.pointers).get(uuid).cloned();
        let root = self.root.clone();
        let id = uuid.clone();
        let (stamp, pointer) = blocking(move || {
            // Taken before the read: a save in between leaves a stamp that no longer
            // matches, so the next request reads again.
            let stamp = super::state::stamp(&root, &id);
            let pointer = match (&cached, stamp) {
                (Some((was, pointer)), Some(now)) if *was == now => pointer.clone(),
                _ => super::state::current_pointer(&root, &id),
            };
            Ok((stamp, pointer))
        })
        .await?;
        let mut pointers = locked(&self.views.pointers);
        match stamp {
            Some(stamp) => {
                pointers.insert(uuid.clone(), (stamp, pointer.clone()));
            }
            None => {
                pointers.remove(uuid);
            }
        }
        Ok(pointer)
    }

    /// The client view of the content image `content_id`, from the cache or its manifest.
    /// Players asking at once for a view not cached yet wait for one computation.
    async fn served(
        &self,
        uuid: &InstanceUuid,
        content_id: &str,
    ) -> Result<Arc<ServedView>, Error> {
        if let Some(view) = locked(&self.views.views).get(&content_id.to_string()) {
            return Ok(view);
        }
        let computing = locked(&self.views.computing)
            .entry(content_id.to_string())
            .or_default()
            .clone();
        let _one = computing.lock().await;
        let result = self.compute_served(uuid, content_id).await;
        locked(&self.views.computing).remove(content_id);
        result
    }

    async fn compute_served(
        &self,
        uuid: &InstanceUuid,
        content_id: &str,
    ) -> Result<Arc<ServedView>, Error> {
        if let Some(view) = locked(&self.views.views).get(&content_id.to_string()) {
            return Ok(view);
        }
        let store = self.store();
        let id = uuid.clone();
        let content = content_id.to_string();
        let view = Arc::new(blocking(move || served_view(&store, &id, &content)).await?);
        self.views.learn(uuid, view.id(), content_id);
        locked(&self.views.views).insert(
            content_id.to_string(),
            view.clone(),
            VIEW_CACHE,
            VIEW_CACHE_BYTES,
            |view| view.json.len(),
        );
        Ok(view)
    }

    /// What to send a player that has the views `have` (client view ids, newest first, at
    /// most [`MAX_HAVE`]): the first one the core knows decides. The current view is
    /// [`ClientAnswer::Current`]; another known view gives a delta from it (or the whole
    /// view when that is no larger); none known gives the whole view. `None` while the
    /// instance has no content image.
    ///
    /// The ids are first looked up among the views the core has learnt; the instance's
    /// manifests are scanned for ids it hasn't seen only when none of those decides, so a
    /// player that keeps a few pruned ids after its current one costs no scan.
    pub async fn client_answer(
        &self,
        uuid: &InstanceUuid,
        have: &[String],
    ) -> Result<Option<ClientAnswer>, Error> {
        let Some(current) = self.client_view(uuid).await? else {
            return Ok(None);
        };
        if let Some(answer) = self.answer_from_known(uuid, have, &current).await? {
            return Ok(Some(answer));
        }
        let unknown = have
            .iter()
            .any(|id| id != current.id() && !self.views.knows(uuid, id));
        if unknown {
            self.learn_view_ids(uuid).await?;
            if let Some(answer) = self.answer_from_known(uuid, have, &current).await? {
                return Ok(Some(answer));
            }
        }
        Ok(Some(ClientAnswer::View(current)))
    }

    /// The answer from the first id of `have` that is the current view or one the core
    /// knows, if any.
    async fn answer_from_known(
        &self,
        uuid: &InstanceUuid,
        have: &[String],
        current: &Arc<ServedView>,
    ) -> Result<Option<ClientAnswer>, Error> {
        for id in have {
            if id == current.id() {
                return Ok(Some(ClientAnswer::Current(current.clone())));
            }
            for content in self.views.contents_of(uuid, id) {
                match self.served(uuid, &content).await {
                    Ok(from) => return Ok(Some(self.delta(from, current.clone()).await?)),
                    // Pruned since it was learnt: another image with the view may still be
                    // there.
                    Err(e) if matches!(e.kind, ErrorKind::NotFound) => {
                        self.views.unlearn(uuid, &content)
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(None)
    }

    /// The answer for a player with the view `from`: a delta to `current`, cached.
    async fn delta(
        &self,
        from: Arc<ServedView>,
        current: Arc<ServedView>,
    ) -> Result<ClientAnswer, Error> {
        if from.id() == current.id() {
            // Another content image with the same client view.
            return Ok(ClientAnswer::Current(current));
        }
        let key = (from.id().to_string(), current.id().to_string());
        let cached = locked(&self.views.deltas).get(&key);
        let json = match cached {
            Some(json) => json,
            None => {
                let to = current.clone();
                let json = blocking(move || Ok(delta_json(&from.view, &to))).await?;
                locked(&self.views.deltas).insert(
                    key,
                    json.clone(),
                    DELTA_CACHE,
                    DELTA_CACHE_BYTES,
                    |json| json.as_ref().map_or(0, Bytes::len),
                );
                json
            }
        };
        Ok(match json {
            Some(json) => ClientAnswer::Delta {
                view: current,
                json,
            },
            None => ClientAnswer::View(current),
        })
    }

    /// Learns the client view id of each content manifest of `uuid` not looked at yet, and
    /// forgets manifests that are gone (a view stays known while any manifest left has it).
    /// Each manifest is read once while the core runs (a view id is a function of the
    /// content id and the rules), and one scan of an instance runs at a time, so a burst of
    /// requests naming unknown views costs one listing of the instance's folder each.
    async fn learn_view_ids(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let lookup = locked(&self.views.lookups)
            .entry(uuid.clone())
            .or_default()
            .clone();
        let _scan = lookup.lock().await;
        let seen: HashSet<String> = locked(&self.views.known)
            .get(uuid)
            .map(|known| known.seen.keys().cloned().collect())
            .unwrap_or_default();
        let store = self.store();
        let links = self.links.clone();
        let id = uuid.clone();
        let (ids, learnt) = blocking(move || {
            let ids = store
                .manifest_ids(id.as_ref())
                .map_err(color_eyre::Report::new)
                .wrap_err("Failed to list the server's images")?;
            let mut learnt = Vec::new();
            for manifest in &ids {
                let data = locked(&links).contains_key(manifest);
                if seen.contains(manifest) || data {
                    continue;
                }
                let view_id = match store.load_header(id.as_ref(), manifest) {
                    Ok(header) if header.kind == Kind::Content => {
                        match served_view(&store, &id, manifest) {
                            Ok(served) => Some(served.view.id),
                            Err(e) => {
                                warn!(
                                    "Failed to read the content image {manifest}: {:#}",
                                    e.source
                                );
                                None
                            }
                        }
                    }
                    Ok(_) => None,
                    Err(e) => {
                        warn!("Failed to read the image {manifest} of {id}: {e}");
                        None
                    }
                };
                learnt.push((manifest.clone(), view_id));
            }
            Ok((ids, learnt))
        })
        .await?;
        let present: HashSet<String> = ids.into_iter().collect();
        let mut known = locked(&self.views.known);
        let known = known.entry(uuid.clone()).or_default();
        let gone: Vec<String> = known
            .seen
            .keys()
            .filter(|id| !present.contains(*id))
            .cloned()
            .collect();
        for manifest in gone {
            known.forget(&manifest);
        }
        for (manifest, view_id) in learnt {
            known.learn(&manifest, view_id.as_deref());
        }
        Ok(())
    }

    /// Records the instance's first content image in the background, once however many
    /// players ask, when it still has none (or its pointer names a manifest that went
    /// missing or can't be read). Unlike the startup sweep it also records a running server
    /// (on one thread), and leaves `running_since` as it is, so the next stop takes the
    /// baseline again as usual. After a record fails, none starts again for
    /// [`FIRST_RECORD_WAIT`], doubling with each failure in a row: players can't make the
    /// core rescan a server (holding its owner's operations off) over and over.
    pub fn record_first(self: &Arc<Self>, ctx: InstanceContext) -> FirstRecord {
        self.record_in_background(ctx, false)
    }

    /// Records the instance's content in the background to store again what the store lost
    /// of its current image (an object a player asked for that was missing or found
    /// damaged): a record stores every file the content stat cache can't claim. Like
    /// [`Self::record_first`], once however many ask, and none starts again for
    /// [`FIRST_RECORD_WAIT`] after one ends, whether it succeeded or not, so a file that
    /// stays missing (or a player that keeps asking) can't make the core rescan the server
    /// over and over.
    pub fn record_again(self: &Arc<Self>, ctx: InstanceContext) -> FirstRecord {
        self.record_in_background(ctx, true)
    }

    fn record_in_background(self: &Arc<Self>, ctx: InstanceContext, again: bool) -> FirstRecord {
        if !Self::accepts(&ctx.uuid) {
            return FirstRecord::Recording;
        }
        let now = self.clock.now();
        let failures = {
            let mut records = locked(&self.views.first_records);
            let failures = match records.get(&ctx.uuid) {
                Some(FirstRecordState::Running) => return FirstRecord::Recording,
                Some(FirstRecordState::Waiting { retry_at, .. }) if *retry_at > now => {
                    return FirstRecord::Failed {
                        retry_after: *retry_at - now,
                    }
                }
                Some(FirstRecordState::Waiting { failures, .. }) => *failures,
                None => 0,
            };
            records.insert(ctx.uuid.clone(), FirstRecordState::Running);
            failures
        };
        let settle = Settle {
            service: self.clone(),
            uuid: ctx.uuid.clone(),
            failures,
            again,
            outcome: None,
        };
        let this = self.clone();
        tokio::spawn(async move {
            let mut settle = settle;
            settle.outcome = Some(this.record_first_now(ctx, again).await);
        });
        FirstRecord::Recording
    }

    async fn record_first_now(&self, ctx: InstanceContext, again: bool) -> Result<(), Error> {
        let slot = self.slot(&ctx.uuid);
        let _gate = slot.gate.clone().write_owned().await;
        let _lock = slot.lock.clone().lock_owned().await;
        if !again {
            let root = self.root.clone();
            let store = self.store();
            let uuid = ctx.uuid.clone();
            let has_image = blocking(move || {
                Ok(super::state::load(&root, &uuid)
                    .current
                    .is_some_and(|id| store.load_manifest(uuid.as_ref(), &id).is_ok()))
            })
            .await?;
            if has_image {
                return Ok(());
            }
        }
        let label = if again {
            super::recorder::OUTSIDE_LABEL
        } else {
            super::recorder::INITIAL_LABEL
        };
        self.record_held(&slot, ctx, Some(label.to_string()), 1)
            .await
            .map(|_| ())
    }

    /// Whether a first record started by [`Self::record_first`] is running.
    #[cfg(test)]
    pub(crate) fn recording_first(&self, uuid: &InstanceUuid) -> bool {
        matches!(
            locked(&self.views.first_records).get(uuid),
            Some(FirstRecordState::Running)
        )
    }
}

/// Settles a background record when its task ends, however it ends: with its outcome, or
/// as failed when the task panicked or was dropped first, so an instance is never left
/// "recording" for good.
struct Settle {
    service: Arc<ImageService>,
    uuid: InstanceUuid,
    /// How many failed in a row before this one.
    failures: u32,
    again: bool,
    outcome: Option<Result<(), Error>>,
}

impl Drop for Settle {
    fn drop(&mut self) {
        let outcome = self.outcome.take().unwrap_or_else(|| {
            Err(Error::from(color_eyre::eyre::eyre!(
                "The record stopped before it ended"
            )))
        });
        let now = self.service.clock.now();
        let mut records = locked(&self.service.views.first_records);
        if !matches!(records.get(&self.uuid), Some(FirstRecordState::Running)) {
            // The instance was deleted meanwhile.
            return;
        }
        match outcome {
            Ok(()) if self.again => {
                records.insert(
                    self.uuid.clone(),
                    FirstRecordState::Waiting {
                        failures: 0,
                        retry_at: now + FIRST_RECORD_WAIT,
                    },
                );
            }
            Ok(()) => {
                records.remove(&self.uuid);
            }
            Err(e) => {
                let failures = self.failures.saturating_add(1);
                let wait = first_record_wait(failures);
                warn!(
                    "Failed to record the content of {} for its players (trying again in {wait:?} at the earliest): {:#}",
                    self.uuid, e.source
                );
                records.insert(
                    self.uuid.clone(),
                    FirstRecordState::Waiting {
                        failures,
                        retry_at: now + wait,
                    },
                );
            }
        }
    }
}

/// The delta from `from` to `to` as sent, or `None` when it is no smaller than the view.
fn delta_json(from: &Image, to: &ServedView) -> Option<Bytes> {
    let changes = delta::diff(from, &to.view);
    // A delta is maps with string keys and plain values: serializing can't fail.
    let json = serde_json::to_vec(&changes).unwrap_or_default();
    (json.len() < to.json.len()).then(|| Bytes::from(json))
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
