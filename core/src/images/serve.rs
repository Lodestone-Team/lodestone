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
//! - which content image each view id comes from, per instance, learned lazily from the
//!   instance's manifests the first time a player names a view the core hasn't seen.
//!
//! A request costs one small read of the pointer (`content-state.json`'s `current`) and then
//! cache hits. The pointer moves only after its manifest is saved, and manifests are
//! written by an atomic rename, so a request never sees a half-written image; the current
//! image and its parent are never pruned, and saving a manifest checks every object it
//! names is in the store, so the objects a current view names are there. Nothing here holds
//! one of the core's locks: a player's request never waits for a record.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex as StdMutex};

use axum::body::Bytes;
use lodestone_image::store::{Store, StoreError};
use lodestone_image::{delta, view, Image, Kind, Side, Source, RULES};
use tracing::warn;

use crate::error::{Error, ErrorKind};
use crate::types::InstanceUuid;

use super::{blocking, ImageService, InstanceContext};

/// The most view ids a player may send in `have`.
pub const MAX_HAVE: usize = 16;
/// The largest view a player may send to ask for a delta from it (`POST …/image`).
pub const MAX_SUMMARY_BYTES: usize = 8 << 20;
/// How many views are kept, by content id: every server's current view and a few older
/// ones players still have.
const VIEW_CACHE: usize = 32;
/// How many deltas are kept, and their bytes in all.
const DELTA_CACHE: usize = 64;
const DELTA_CACHE_BYTES: usize = 16 << 20;

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
    /// The whole current view: the player has none the core knows, or the delta would be
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

/// Which content image each client view id comes from, for one instance.
#[derive(Default)]
struct Known {
    /// Client view id to a content image id with that view.
    views: HashMap<String, String>,
    /// Manifests already looked at (content or not), so each is read once.
    seen: HashSet<String>,
}

/// The caches of what players are served (see the module). Part of [`ImageService`].
pub(crate) struct ViewCache {
    views: StdMutex<Bounded<String, Arc<ServedView>>>,
    /// `None`: the delta is no smaller than the view, which is sent instead.
    deltas: StdMutex<Bounded<(String, String), Option<Bytes>>>,
    known: StdMutex<HashMap<InstanceUuid, Known>>,
    /// One scan of an instance's manifests at a time.
    lookups: StdMutex<HashMap<InstanceUuid, Arc<tokio::sync::Mutex<()>>>>,
    /// Instances whose first content image a player's request is recording.
    first_records: StdMutex<HashSet<InstanceUuid>>,
}

impl Default for ViewCache {
    fn default() -> ViewCache {
        ViewCache {
            views: StdMutex::new(Bounded::new()),
            deltas: StdMutex::new(Bounded::new()),
            known: Default::default(),
            lookups: Default::default(),
            first_records: Default::default(),
        }
    }
}

fn locked<T>(mutex: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl ViewCache {
    /// Forgets a deleted instance's view ids. Its views and deltas age out.
    pub(crate) fn forget(&self, uuid: &InstanceUuid) {
        locked(&self.known).remove(uuid);
        locked(&self.lookups).remove(uuid);
    }

    fn content_of(&self, uuid: &InstanceUuid, view_id: &str) -> Option<String> {
        locked(&self.known)
            .get(uuid)
            .and_then(|known| known.views.get(view_id).cloned())
    }

    fn learn(&self, uuid: &InstanceUuid, view_id: &str, content_id: &str) {
        let mut known = locked(&self.known);
        let known = known.entry(uuid.clone()).or_default();
        known
            .views
            .insert(view_id.to_string(), content_id.to_string());
        known.seen.insert(content_id.to_string());
    }
}

/// The client view of the content image `content_id` of `uuid`, from its manifest (which
/// is checked against its id as it is read). Blocking.
fn served_view(store: &Store, uuid: &InstanceUuid, content_id: &str) -> Result<ServedView, Error> {
    let image = store
        .load_manifest(uuid.as_ref(), content_id)
        .map_err(|e| match e {
            StoreError::Missing(_) => Error {
                kind: ErrorKind::NotFound,
                source: color_eyre::eyre::eyre!("The server's content image is gone"),
            },
            e => Error::from(color_eyre::eyre::eyre!(
                "Failed to read the content image {content_id} of {uuid}: {e}"
            )),
        })?;
    let view = view(&image, Side::Client, RULES).map_err(|e| {
        Error::from(color_eyre::eyre::eyre!(
            "Failed to compute the client view of {content_id}: {e}"
        ))
    })?;
    Ok(ServedView::new(content_id.to_string(), view))
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: color_eyre::eyre::eyre!(message.into()),
    }
}

/// Whether `image` is a client view whose id is its own (the spec's check for a view
/// received from elsewhere: its id equals the id of its own view).
fn check_client_view(image: &Image) -> Result<(), Error> {
    if image.side != Some(Side::Client) {
        return Err(bad_request("This isn't a client view"));
    }
    match view(image, Side::Client, image.rules) {
        Ok(again) if again.id == image.id => Ok(()),
        Ok(_) => Err(bad_request("The view's id doesn't match its contents")),
        Err(e) => Err(bad_request(format!("The view can't be read: {e}"))),
    }
}

impl ImageService {
    /// The client view of the instance's current content image, or `None` while it has
    /// none (start one with [`Self::record_first`]).
    pub async fn client_view(&self, uuid: &InstanceUuid) -> Result<Option<Arc<ServedView>>, Error> {
        if !Self::accepts(uuid) {
            return Err(Error {
                kind: ErrorKind::NotFound,
                source: color_eyre::eyre::eyre!("Server not found"),
            });
        }
        let root = self.root.clone();
        let id = uuid.clone();
        let Some(current) = blocking(move || Ok(super::state::current_pointer(&root, &id))).await?
        else {
            return Ok(None);
        };
        match self.served(uuid, &current).await {
            Ok(view) => Ok(Some(view)),
            // A pointer whose manifest went missing: the next record saves it again.
            Err(e) if matches!(e.kind, ErrorKind::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The client view of the content image `content_id`, from the cache or its manifest.
    async fn served(
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
            usize::MAX,
            |_| 0,
        );
        Ok(view)
    }

    /// What to send a player that has the views `have` (client view ids, newest first, at
    /// most [`MAX_HAVE`]): the first one the core knows decides. The current view is
    /// [`ClientAnswer::Current`]; another known view gives a delta from it (or the whole
    /// view when that is no larger); none known gives the whole view. `None` while the
    /// instance has no content image.
    pub async fn client_answer(
        &self,
        uuid: &InstanceUuid,
        have: &[String],
    ) -> Result<Option<ClientAnswer>, Error> {
        let Some(current) = self.client_view(uuid).await? else {
            return Ok(None);
        };
        let unknown = have
            .iter()
            .any(|id| id != current.id() && self.views.content_of(uuid, id).is_none());
        if unknown {
            self.learn_view_ids(uuid).await?;
        }
        for id in have {
            if id == current.id() {
                return Ok(Some(ClientAnswer::Current(current)));
            }
            let Some(content) = self.views.content_of(uuid, id) else {
                continue;
            };
            match self.served(uuid, &content).await {
                Ok(from) => return Ok(Some(self.delta(from, current).await?)),
                // Pruned since it was learnt: as if it were unknown.
                Err(e) if matches!(e.kind, ErrorKind::NotFound) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(Some(ClientAnswer::View(current)))
    }

    /// What to send a player whose views were all pruned: it sends the client view it last
    /// applied (`have_view`, which must be a client view whose id checks out, else a bad
    /// request), and gets a delta from it (or [`ClientAnswer::Current`], or the whole view
    /// when that is no larger). A view rather than bare paths and hashes, since a delta must
    /// give the exact image: every entry's source, sides and mode. Not cached: such a view
    /// is the player's own. `None` while the instance has no content image.
    pub async fn client_answer_from(
        &self,
        uuid: &InstanceUuid,
        have_view: Image,
    ) -> Result<Option<ClientAnswer>, Error> {
        let have_view = blocking(move || check_client_view(&have_view).map(|()| have_view)).await?;
        let Some(current) = self.client_view(uuid).await? else {
            return Ok(None);
        };
        if have_view.id == current.id() {
            return Ok(Some(ClientAnswer::Current(current)));
        }
        let to = current.clone();
        let json = blocking(move || Ok(delta_json(&have_view, &to))).await?;
        Ok(Some(match json {
            Some(json) => ClientAnswer::Delta {
                view: current,
                json,
            },
            None => ClientAnswer::View(current),
        }))
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
    /// forgets those of manifests that are gone. Each manifest is read once while the core
    /// runs (a view id is a function of the content id and the rules), and one scan of an
    /// instance runs at a time, so a burst of requests naming unknown views costs one
    /// listing of the instance's folder each.
    async fn learn_view_ids(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let lookup = locked(&self.views.lookups)
            .entry(uuid.clone())
            .or_default()
            .clone();
        let _scan = lookup.lock().await;
        let seen: HashSet<String> = locked(&self.views.known)
            .get(uuid)
            .map(|known| known.seen.clone())
            .unwrap_or_default();
        let store = self.store();
        let links = self.links.clone();
        let id = uuid.clone();
        let (ids, learnt) = blocking(move || {
            let ids = store
                .manifest_ids(id.as_ref())
                .map_err(|e| color_eyre::eyre::eyre!("Failed to list the images: {e}"))?;
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
                                warn!("Failed to read the content image {manifest}: {}", e.source);
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
        known.seen.retain(|id| present.contains(id));
        known.views.retain(|_, content| present.contains(content));
        for (manifest, view_id) in learnt {
            if let Some(view_id) = view_id {
                known.views.insert(view_id, manifest.clone());
            }
            known.seen.insert(manifest);
        }
        Ok(())
    }

    /// Records the instance's first content image in the background, once however many
    /// players ask, when it still has none (or its pointer names a manifest that went
    /// missing). Unlike the startup sweep it also records a running server (on one
    /// thread), and leaves `running_since` as it is, so the next stop takes the baseline
    /// again as usual.
    pub fn record_first(self: &Arc<Self>, ctx: InstanceContext) {
        if !Self::accepts(&ctx.uuid) || !locked(&self.views.first_records).insert(ctx.uuid.clone())
        {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let uuid = ctx.uuid.clone();
            if let Err(e) = this.record_first_now(ctx).await {
                warn!(
                    "Failed to record the first content image of {uuid}: {}",
                    e.source
                );
            }
            locked(&this.views.first_records).remove(&uuid);
        });
    }

    async fn record_first_now(&self, ctx: InstanceContext) -> Result<(), Error> {
        let slot = self.slot(&ctx.uuid);
        let _gate = slot.gate.clone().write_owned().await;
        let _lock = slot.lock.clone().lock_owned().await;
        let root = self.root.clone();
        let store = self.store();
        let uuid = ctx.uuid.clone();
        let has_image = blocking(move || {
            Ok(super::state::load(&root, &uuid).current.is_some_and(|id| {
                store
                    .manifest_path(uuid.as_ref(), &id)
                    .is_ok_and(|path| path.is_file())
            }))
        })
        .await?;
        if has_image {
            return Ok(());
        }
        self.record_held(
            &slot,
            ctx,
            Some(super::recorder::INITIAL_LABEL.to_string()),
            1,
        )
        .await
        .map(|_| ())
    }

    /// Whether a first record started by [`Self::record_first`] is running.
    #[cfg(test)]
    pub(crate) fn recording_first(&self, uuid: &InstanceUuid) -> bool {
        locked(&self.views.first_records).contains(uuid)
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
