//! Instances for Lodestone Launcher:
//!
//! - `GET /public/instances` and `GET /public/instances/{uuid}/client-pack`: the listed
//!   instances and their client modpacks, without signing in, while the owner allows it;
//! - `GET /launcher/instances` and `GET /launcher/instances/{uuid}/client-pack`: the same
//!   for the instances a signed-in user (or launcher key) can view;
//! - `GET …/instances/{uuid}/image` and `GET …/instances/{uuid}/objects/{sha256}`
//!   under both prefixes: what a player needs to join as an instance image (the client view
//!   of the server's content image, deltas to it and the bytes only the server has, see
//!   [`super::player_image`]), with the client pack's access rules;
//! - `GET`/`PUT /instance/{uuid}/public_listing`: whether an instance is listed, and
//!   what it shows (its [`PublicScope`]); only the owner changes it;
//! - `GET /public_listing`: the scope of every instance, for the owner.
//!
//! The public routes give out only what a player needs to join and the instance's scope
//! allows (see [`PublicInstance`]), are cached, and answer "not found" alike for
//! missing, unlisted and hidden instances.

use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{
        header::{
            CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH,
        },
        HeaderMap, Method, StatusCode,
    },
    response::Response,
    routing::get,
    Extension, Json, Router,
};
use color_eyre::eyre::{eyre, Context};
use tokio_util::io::ReaderStream;

use super::instance::sanitize_instance_dir_name;
use super::instance_mods::{fabric_loader_version, mod_folder};
use super::player_image::{image_response, object_response, ImageQuery};
use super::util::{all_instances, get_instance, AuthedUser};
use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::{Event, EventInner, InstanceEvent, InstanceEventInner},
    images::objects::ClientKey,
    mods::{bad_request, client::build_client_pack, InstalledModLoader},
    prelude::GameInstance,
    public_listing::{
        has_enabled_jar, pack_fingerprint, sha256_of, BuiltPack, CachedPack, InstancePublicListing,
        InstancePublicScope, PackSlot, PublicInstance, PublicInstanceList, PublicListingOverview,
        PublicLoader, PublicScope, PublicScopeChange,
    },
    traits::{
        t_configurable::{Game, MinecraftVariant, TConfigurable},
        TInstance,
    },
    types::{InstanceUuid, Snowflake},
    AppState,
};

/// The media type of a Modrinth modpack.
const MRPACK_TYPE: &str = "application/x-modrinth-modpack+zip";

fn not_found() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    }
}

fn loader_of(variant: &MinecraftVariant) -> PublicLoader {
    match variant {
        MinecraftVariant::Vanilla => PublicLoader::Vanilla,
        MinecraftVariant::Fabric => PublicLoader::Fabric,
        MinecraftVariant::Forge => PublicLoader::Forge,
        MinecraftVariant::NeoForge => PublicLoader::NeoForge,
        MinecraftVariant::Paper => PublicLoader::Paper,
        MinecraftVariant::Spigot => PublicLoader::Spigot,
        MinecraftVariant::Other { .. } => PublicLoader::Other,
    }
}

/// Whether players can join the instance with Minecraft Java Edition.
async fn is_minecraft_java(instance: &GameInstance) -> bool {
    matches!(instance.game_type().await, Game::MinecraftJava { .. })
}

/// An instance as [`PublicInstance`] with the fields `scope` shows, or `None` if it is
/// not a Minecraft Java instance. `scope.listed` is only reported, not checked.
pub(crate) async fn public_instance(
    instance: &GameInstance,
    scope: PublicScope,
) -> Option<PublicInstance> {
    let info = instance.get_instance_info().await;
    let Game::MinecraftJava { variant } = &info.game_type else {
        return None;
    };
    let loader = loader_of(variant);
    let has_client_pack = if scope.client_pack {
        Some(
            loader == PublicLoader::Fabric
                && has_enabled_jar(instance.path().await.join("mods")).await,
        )
    } else {
        None
    };
    let status = scope.status;
    let address = scope.address;
    Some(PublicInstance {
        uuid: info.uuid,
        name: info.name,
        game_version: info.version,
        loader,
        loader_version: info.loader_version,
        port: address.then_some(info.port),
        state: status.then_some(info.state),
        player_count: status.then_some(info.player_count),
        max_player_count: status.then_some(info.max_player_count),
        has_client_pack,
        listed: scope.listed,
    })
}

/// The instances `include` gives a scope, oldest first, each showing what its scope
/// allows.
async fn instance_list(
    state: &AppState,
    include: impl Fn(&InstanceUuid) -> Option<PublicScope>,
) -> PublicInstanceList {
    let (core_name, listing_enabled) = {
        let settings = state.global_settings.lock().await;
        (settings.core_name(), settings.list_instances_publicly())
    };
    let mut chosen = Vec::new();
    for instance in all_instances(state) {
        let uuid = instance.uuid().await;
        if let Some(scope) = include(&uuid) {
            chosen.push((instance.creation_time().await, scope, instance));
        }
    }
    chosen.sort_by_key(|(created, _, _)| *created);
    let mut instances = Vec::new();
    for (_, scope, instance) in chosen {
        if let Some(entry) = public_instance(&instance, scope).await {
            instances.push(entry);
        }
    }
    PublicInstanceList {
        core_name,
        core_uuid: state.uuid.clone(),
        listing_enabled,
        instances,
    }
}

/// What anyone may see of the instance: its scope while the switch is on and it is
/// listed, else `None`.
async fn public_scope(state: &AppState, uuid: &InstanceUuid) -> Option<PublicScope> {
    let scope = state.public_listing.scope(uuid);
    (state.global_settings.lock().await.list_instances_publicly() && scope.listed).then_some(scope)
}

async fn get_public_instances(State(state): State<AppState>) -> Json<PublicInstanceList> {
    if let Some(list) = state.public_listing.cached_list().await {
        return Json(list);
    }
    let enabled = state.global_settings.lock().await.list_instances_publicly();
    let listing = state.public_listing.clone();
    let list = instance_list(&state, |uuid| {
        Some(listing.scope(uuid)).filter(|scope| enabled && scope.listed)
    })
    .await;
    state.public_listing.store_list(list.clone()).await;
    Json(list)
}

async fn get_launcher_instances(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Json<PublicInstanceList> {
    Json(
        // the requester's permissions decide, not the public scope
        instance_list(&state, |uuid| {
            requester
                .can_perform_action(&UserAction::ViewInstance(uuid.clone()))
                .then(|| PublicScope {
                    listed: state.public_listing.is_listed(uuid),
                    ..PublicScope::ALL
                })
        })
        .await,
    )
}

/// The client pack of the instance, from the cache when its mods folder has not changed.
async fn client_pack(
    state: &AppState,
    uuid: &InstanceUuid,
) -> Result<std::sync::Arc<BuiltPack>, Error> {
    let fabric_only = || bad_request("Client modpacks are only available for Fabric servers");
    // vanilla instances have no mods folder at all
    let (instance, folder) = mod_folder(state, uuid).await.map_err(|e| match e.kind {
        ErrorKind::BadRequest => fabric_only(),
        _ => e,
    })?;
    if folder.loader != InstalledModLoader::Fabric {
        return Err(fabric_only());
    }
    let name = instance.name().await;
    let loader_version = fabric_loader_version(&instance).await;
    let fingerprint = pack_fingerprint(
        folder.dir.clone(),
        name.clone(),
        folder.game_version.clone(),
        loader_version.clone(),
    )
    .await?;
    let slot = state.public_listing.pack_slot(uuid);
    let mut slot = slot.lock().await;
    if let Some(current) = slot.as_ref().filter(|s| s.fingerprint == fingerprint) {
        return match &current.pack {
            CachedPack::Built(pack) => Ok(pack.clone()),
            CachedPack::Failed(kind, message) => Err(Error {
                kind: kind.clone(),
                source: eyre!("{message}"),
            }),
        };
    }
    let built = build_pack(state, &folder, &name, loader_version.as_deref()).await;
    *slot = cached_outcome(&built).map(|pack| PackSlot { fingerprint, pack });
    built
}

/// What to keep of a build for later requests with the same mods folder: the pack, or a
/// failure that building again would only repeat (such as a server whose players need no
/// mods). `None` for a failure that may pass: Modrinth unreachable or rate limited (which
/// is `External`), or a file that couldn't be read or written (`Internal`). The next
/// request then tries again, as the message tells people to.
fn cached_outcome(built: &Result<std::sync::Arc<BuiltPack>, Error>) -> Option<CachedPack> {
    match built {
        Ok(pack) => Some(CachedPack::Built(pack.clone())),
        Err(e) => match e.kind {
            ErrorKind::External | ErrorKind::Internal | ErrorKind::TooManyRequests => None,
            _ => Some(CachedPack::Failed(e.kind.clone(), e.source.to_string())),
        },
    }
}

async fn build_pack(
    state: &AppState,
    folder: &crate::mods::ModFolder,
    name: &str,
    loader_version: Option<&str>,
) -> Result<std::sync::Arc<BuiltPack>, Error> {
    let pack_dir = state.public_listing.pack_dir();
    tokio::fs::create_dir_all(pack_dir)
        .await
        .context("Failed to create the folder for client modpacks")?;
    let temp_dir = tempfile::tempdir_in(pack_dir).context("Failed to create a temporary folder")?;
    let file_name = format!("{}-client.mrpack", sanitize_instance_dir_name(name));
    let out = temp_dir.path().join(&file_name);
    let plan = build_client_pack(
        state.mods.modrinth.as_ref(),
        folder,
        name,
        loader_version,
        &out,
    )
    .await?;
    let sha256 = sha256_of(out.clone()).await?;
    Ok(std::sync::Arc::new(BuiltPack::new(
        temp_dir,
        out,
        file_name,
        sha256,
        (plan.index.files.len() + plan.included.len()) as u32,
        plan.included.len() as u32,
    )))
}

/// Whether `If-None-Match` names `etag`.
fn matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|v| v.trim().trim_start_matches("W/"))
        .any(|v| v == etag || v == "*")
}

/// The pack as a response: 304 when the client has it already, else the file.
pub(crate) async fn pack_response(
    state: &AppState,
    uuid: &InstanceUuid,
    headers: &HeaderMap,
) -> Result<Response, Error> {
    let pack = client_pack(state, uuid).await?;
    let etag = pack.etag();
    let response = Response::builder()
        .header(ETAG, &etag)
        .header(CACHE_CONTROL, "no-cache")
        .header("X-Lodestone-Mod-Count", pack.mod_count)
        .header("X-Lodestone-Included-Jars", pack.included_jars);
    if matches_etag(headers, &etag) {
        return Ok(response
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .context("Failed to build the response")?);
    }
    // opened before the pack can be replaced, which deletes its file
    let file = tokio::fs::File::open(&pack.path)
        .await
        .context("Failed to open the client modpack")?;
    let mut response = response.header(CONTENT_TYPE, MRPACK_TYPE).header(
        CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"{}\"",
            pack.file_name.replace('"', "_")
        ),
    );
    if let Ok(metadata) = file.metadata().await {
        response = response.header(CONTENT_LENGTH, metadata.len());
    }
    Ok(response
        .body(Body::from_stream(ReaderStream::new(file)))
        .context("Failed to build the response")?)
}

/// Whether anyone may fetch what players need of the instance (its client pack and image):
/// listed with the client-pack scope while the switch is on, and there. "Not found" alike
/// otherwise.
async fn public_allows(state: &AppState, uuid: &InstanceUuid) -> Result<(), Error> {
    let shared = public_scope(state, uuid)
        .await
        .is_some_and(|scope| scope.client_pack);
    if !shared || !state.instances.contains_key(uuid) {
        return Err(not_found());
    }
    Ok(())
}

/// Whether `requester` may fetch what players need of the instance: they can view it.
async fn launcher_allows(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
) -> Result<(), Error> {
    // checked first, so the reply does not show whether the instance exists
    requester
        .require(state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(state, uuid)?;
    Ok(())
}

async fn get_public_client_pack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    public_allows(&state, &uuid).await?;
    pack_response(&state, &uuid, &headers).await
}

async fn get_launcher_client_pack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    headers: HeaderMap,
) -> Result<Response, Error> {
    launcher_allows(&state, &requester, &uuid).await?;
    pack_response(&state, &uuid, &headers).await
}

async fn get_public_image(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    Query(query): Query<ImageQuery>,
) -> Result<Response, Error> {
    public_allows(&state, &uuid).await?;
    image_response(&state, &uuid, query).await
}

/// Who asks for an object, for the per-client download limit: the peer's address, which
/// the main port's server gives every request (see `crate::images::objects::ClientKey`).
fn client_of(peer: Option<Extension<ConnectInfo<SocketAddr>>>) -> Option<ClientKey> {
    peer.map(|Extension(ConnectInfo(addr))| ClientKey::of_address(addr.ip()))
}

async fn get_public_object(
    State(state): State<AppState>,
    Path((uuid, sha256)): Path<(InstanceUuid, String)>,
    method: Method,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> Result<Response, Error> {
    public_allows(&state, &uuid).await?;
    object_response(&state, &uuid, &sha256, &method, &headers, client_of(peer)).await
}

async fn get_launcher_image(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    // Before the sign-in: a bad query is refused before anything else, as on every route.
    Query(query): Query<ImageQuery>,
    requester: AuthedUser,
) -> Result<Response, Error> {
    launcher_allows(&state, &requester, &uuid).await?;
    image_response(&state, &uuid, query).await
}

async fn get_launcher_object(
    State(state): State<AppState>,
    Path((uuid, sha256)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    method: Method,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> Result<Response, Error> {
    launcher_allows(&state, &requester, &uuid).await?;
    object_response(&state, &uuid, &sha256, &method, &headers, client_of(peer)).await
}

async fn get_public_listing(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstancePublicListing>, Error> {
    // anyone who can see the instance sees what it shows publicly; only the owner changes it
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?;
    Ok(Json(InstancePublicListing {
        scope: state.public_listing.scope(&uuid),
        list_instances_publicly: state.global_settings.lock().await.list_instances_publicly(),
    }))
}

/// `GET /public_listing`, for the owner: every Minecraft Java instance, oldest first.
async fn get_public_listing_overview(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<PublicListingOverview>, Error> {
    requester.require_owner()?;
    let mut instances = Vec::new();
    for instance in all_instances(&state) {
        if !is_minecraft_java(&instance).await {
            continue;
        }
        let uuid = instance.uuid().await;
        instances.push((
            instance.creation_time().await,
            InstancePublicScope {
                scope: state.public_listing.scope(&uuid),
                name: instance.name().await,
                uuid,
            },
        ));
    }
    // Creation times are in seconds, so instances made in the same second are ordered by
    // name, then uuid: the order never depends on the instance map's iteration order.
    instances.sort_by(|(a_created, a), (b_created, b)| {
        a_created
            .cmp(b_created)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.uuid.to_string().cmp(&b.uuid.to_string()))
    });
    Ok(Json(PublicListingOverview {
        list_instances_publicly: state.global_settings.lock().await.list_instances_publicly(),
        instances: instances.into_iter().map(|(_, entry)| entry).collect(),
    }))
}

/// The body of `PUT /instance/{uuid}/public_listing`: a bare boolean lists or unlists
/// (as before scopes), an object changes the flags it names.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ListingRequest {
    Listed(bool),
    Change(PublicScopeChange),
}

impl ListingRequest {
    fn into_change(self) -> PublicScopeChange {
        match self {
            ListingRequest::Listed(listed) => PublicScopeChange {
                listed: Some(listed),
                ..Default::default()
            },
            ListingRequest::Change(change) => change,
        }
    }
}

/// The rule for changing what an instance shows publicly: only the owner, and the global
/// switch for listing an instance that is not listed yet.
fn check_change(
    requester: &User,
    current: PublicScope,
    change: &PublicScopeChange,
    switch_on: bool,
) -> Result<(), Error> {
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Only the owner can change whether a server is public"),
        });
    }
    if change.listed == Some(true) && !current.listed && !switch_on {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Listing servers publicly is turned off in Core Settings"),
        });
    }
    Ok(())
}

/// What changed in what an instance shows, for the event log: "listed publicly (status,
/// address)", "no longer listed publicly", or the flags of a listed (or unlisted) one.
fn describe(before: PublicScope, scope: PublicScope) -> String {
    let shown = shown(scope);
    match (before.listed, scope.listed) {
        (false, true) => format!("listed publicly ({shown})"),
        (true, false) => "no longer listed publicly".to_string(),
        (true, true) => format!("now shows publicly: {shown}"),
        (false, false) => format!("will show publicly, once listed: {shown}"),
    }
}

/// The flags of `scope` that are on: "status, address", or "name and version only".
fn shown(scope: PublicScope) -> String {
    let shown: Vec<&str> = [
        (scope.status, "status"),
        (scope.address, "address"),
        (scope.client_pack, "client mods"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if shown.is_empty() {
        "name and version only".to_string()
    } else {
        shown.join(", ")
    }
}

async fn set_public_listing(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ListingRequest>,
) -> Result<Json<PublicScope>, Error> {
    let change = request.into_change();
    let switch_on = state.global_settings.lock().await.list_instances_publicly();
    check_change(
        &requester,
        state.public_listing.scope(&uuid),
        &change,
        switch_on,
    )?;
    let instance = get_instance(&state, &uuid)?;
    if !is_minecraft_java(&instance).await {
        return Err(bad_request(
            "Only Minecraft Java Edition servers can be listed publicly",
        ));
    }
    let before = state.public_listing.scope(&uuid);
    let scope = state.public_listing.change(&uuid, &change).await?;
    if scope != before {
        // about this instance, so only those who can view it see it (with its name)
        let name = instance.name().await;
        state.event_broadcaster.send(Event {
            details: format!("{name} {}", describe(before, scope)),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: uuid.clone(),
                instance_name: name,
                instance_event_inner: InstanceEventInner::PublicListingChanged { scope },
            }),
            snowflake: Snowflake::default(),
            caused_by: requester.caused_by(),
        });
    }
    Ok(Json(scope))
}

pub fn get_public_instance_routes(state: AppState) -> Router {
    Router::new()
        .route("/public/instances", get(get_public_instances))
        .route(
            "/public/instances/{uuid}/client-pack",
            get(get_public_client_pack),
        )
        .route("/launcher/instances", get(get_launcher_instances))
        .route(
            "/launcher/instances/{uuid}/client-pack",
            get(get_launcher_client_pack),
        )
        .route("/public/instances/{uuid}/image", get(get_public_image))
        .route(
            "/public/instances/{uuid}/objects/{sha256}",
            get(get_public_object),
        )
        .route("/launcher/instances/{uuid}/image", get(get_launcher_image))
        .route(
            "/launcher/instances/{uuid}/objects/{sha256}",
            get(get_launcher_object),
        )
        .route(
            "/instance/{uuid}/public_listing",
            get(get_public_listing).put(set_public_listing),
        )
        .route("/public_listing", get(get_public_listing_overview))
        .with_state(state)
}

#[cfg(test)]
mod tests;
