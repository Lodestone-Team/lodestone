//! Instances for Lodestone Launcher:
//!
//! - `GET /public/instances` and `GET /public/instances/{uuid}/client-pack`: the listed
//!   instances and their client modpacks, without signing in, while the owner allows it;
//! - `GET /launcher/instances` and `GET /launcher/instances/{uuid}/client-pack`: the same
//!   for the instances a signed-in user (or launcher key) can view;
//! - `GET`/`PUT /instance/{uuid}/public_listing`: whether an instance is listed, and
//!   what it shows (its [`PublicScope`]); only the owner changes it;
//! - `GET /public_listing`: the scope of every instance, for the owner.
//!
//! The public routes give out only what a player needs to join and the instance's scope
//! allows (see [`PublicInstance`]), are cached, and answer "not found" alike for
//! missing, unlisted and hidden instances.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{
            CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH,
        },
        HeaderMap, StatusCode,
    },
    response::Response,
    routing::get,
    Json, Router,
};
use color_eyre::eyre::{eyre, Context};
use tokio_util::io::ReaderStream;
use tracing::warn;

use super::instance::sanitize_instance_dir_name;
use super::instance_mods::{fabric_loader_version, mod_folder};
use super::util::{all_instances, get_instance, AuthedUser};
use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::{Event, EventInner},
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
        source: eyre!("Instance not found"),
    }
}

fn loader_of(variant: &MinecraftVariant) -> PublicLoader {
    match variant {
        MinecraftVariant::Vanilla => PublicLoader::Vanilla,
        MinecraftVariant::Fabric => PublicLoader::Fabric,
        MinecraftVariant::Forge => PublicLoader::Forge,
        MinecraftVariant::Paper => PublicLoader::Paper,
        MinecraftVariant::Spigot => PublicLoader::Spigot,
        MinecraftVariant::Other { .. } => PublicLoader::Other,
    }
}

/// Whether players can join the instance with Minecraft Java Edition.
async fn is_minecraft_java(instance: &GameInstance) -> bool {
    matches!(instance.game_type().await, Game::MinecraftJava { .. })
}

/// The playit.gg addresses by local port, if the core has a playit key and playit is on.
async fn playit_addresses(state: &AppState) -> Vec<(u16, String)> {
    if !state.global_settings.lock().await.playit_enabled() {
        return Vec::new();
    }
    let Some(secret) = state.playitgg_key.lock().await.clone() else {
        return Vec::new();
    };
    state
        .public_listing
        .playit_addresses(|| async move {
            match crate::playitgg::fetch_tunnels(secret).await {
                Ok(tunnels) => tunnels
                    .into_iter()
                    .filter(|t| t.active)
                    .map(|t| (t.local_port, t.server_address))
                    .collect(),
                Err(e) => {
                    warn!(
                        "Couldn't list the playit.gg tunnels for the instance list: {}",
                        e.source
                    );
                    Vec::new()
                }
            }
        })
        .await
}

/// An instance as [`PublicInstance`] with the fields `scope` shows, or `None` if it is
/// not a Minecraft Java instance. `scope.listed` is only reported, not checked.
async fn public_instance(
    instance: &GameInstance,
    scope: PublicScope,
    playit: &[(u16, String)],
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
    let playit_address = playit
        .iter()
        .find(|(port, _)| u32::from(*port) == info.port)
        .map(|(_, address)| address.clone());
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
        playit_address: address.then_some(playit_address),
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
    // fetched only when an address is shown
    let playit = if chosen.iter().any(|(_, scope, _)| scope.address) {
        playit_addresses(state).await
    } else {
        Vec::new()
    };
    let mut instances = Vec::new();
    for (_, scope, instance) in chosen {
        if let Some(entry) = public_instance(&instance, scope, &playit).await {
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
    let pack = match &built {
        Ok(pack) => CachedPack::Built(pack.clone()),
        Err(e) => CachedPack::Failed(e.kind.clone(), e.source.to_string()),
    };
    *slot = Some(PackSlot { fingerprint, pack });
    built
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
async fn pack_response(
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

async fn get_public_client_pack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let shared = public_scope(&state, &uuid)
        .await
        .is_some_and(|scope| scope.client_pack);
    if !shared || !state.instances.contains_key(&uuid) {
        return Err(not_found());
    }
    pack_response(&state, &uuid, &headers).await
}

async fn get_launcher_client_pack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    headers: HeaderMap,
) -> Result<Response, Error> {
    // checked first, so the reply does not show whether the instance exists
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?;
    pack_response(&state, &uuid, &headers).await
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
    instances.sort_by_key(|(created, _)| *created);
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
            source: eyre!("Only the owner can change whether an instance is public"),
        });
    }
    if change.listed == Some(true) && !current.listed && !switch_on {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Listing instances publicly is turned off in Core Settings"),
        });
    }
    Ok(())
}

/// What an instance shows, for the event log: "listed publicly (status, address)".
fn describe(scope: PublicScope) -> String {
    if !scope.listed {
        return "no longer listed publicly".to_string();
    }
    let shown: Vec<&str> = [
        (scope.status, "status"),
        (scope.address, "address"),
        (scope.client_pack, "client mods"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if shown.is_empty() {
        "listed publicly (name and version only)".to_string()
    } else {
        format!("listed publicly ({})", shown.join(", "))
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
            "Only Minecraft Java Edition instances can be listed publicly",
        ));
    }
    let before = state.public_listing.scope(&uuid);
    let scope = state.public_listing.change(&uuid, &change).await?;
    if scope != before {
        state.event_broadcaster.send(Event {
            event_inner: EventInner::GlobalSettingsChanged,
            details: format!("{} {}", instance.name().await, describe(scope)),
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
        .route(
            "/instance/{uuid}/public_listing",
            get(get_public_listing).put(set_public_listing),
        )
        .route("/public_listing", get(get_public_listing_overview))
        .with_state(state)
}

#[cfg(test)]
mod tests;
