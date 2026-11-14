use super::util::{AuthedUser, ClientIp};
use axum::{
    routing::{get, put},
    Json, Router,
};
use color_eyre::eyre::eyre;
use dashmap::DashMap;
use tracing::{info, warn};

use crate::{
    error::ErrorKind,
    events::{CausedBy, Event, EventInner},
    prelude::GameInstance,
    traits::t_server::{State, TServer},
    types::{InstanceUuid, Snowflake},
    AppState, Error, GlobalSettingsData,
};

pub async fn get_core_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    _requester: AuthedUser,
) -> Result<Json<GlobalSettingsData>, Error> {
    Ok(Json(state.global_settings.lock().await.client_view()))
}

pub async fn change_core_name(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(new_name): Json<String>,
) -> Result<(), Error> {
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core name"),
        });
    }
    if new_name.len() > 32 {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name too long"),
        });
    }
    if new_name.is_empty() {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name cannot be empty"),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_core_name(new_name)
        .await?;
    Ok(())
}

pub async fn change_core_safe_mode(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(safe_mode): Json<bool>,
) -> Result<(), Error> {
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core safe mode"),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_safe_mode(safe_mode)
        .await?;
    Ok(())
}

/// Turns code execution (custom Java commands) on or off, and tells connected clients to
/// fetch the settings again.
pub async fn change_code_execution(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("turn code execution on or off")?;
    state
        .global_settings
        .lock()
        .await
        .set_code_execution_enabled(enabled)
        .await?;
    info!(
        "Code execution turned {}",
        if enabled { "on" } else { "off" }
    );
    if !enabled {
        stop_custom_command_lines(&state.instances, requester.caused_by()).await;
    }
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details: format!(
            "Code execution turned {}",
            if enabled { "on" } else { "off" }
        ),
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

/// Stops the servers that run a custom Java command or Java arguments, which may only run
/// while code execution is on; they can't start again until it is turned back on. Each is
/// asked to stop, which saves its world, as a user's stop would.
pub(crate) async fn stop_custom_command_lines(
    instances: &DashMap<InstanceUuid, GameInstance>,
    caused_by: CausedBy,
) {
    for (uuid, instance) in crate::snapshot_instances(instances) {
        if !matches!(instance.state().await, State::Starting | State::Running) {
            continue;
        }
        let GameInstance::MinecraftInstance(minecraft) = &instance;
        if !minecraft.has_custom_command_line().await {
            continue;
        }
        info!("Stopping instance {uuid}: its custom Java command line needs code execution");
        if let Err(e) = instance.stop(caused_by.clone(), false).await {
            warn!("Failed to stop instance {uuid} after code execution was turned off: {e}");
        }
    }
}

/// Allows or refuses plain HTTP from other devices. Takes effect for new connections;
/// connections that are already open stay open.
pub async fn change_allow_lan_http(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(allow_lan_http): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("allow or refuse plain HTTP from other devices")?;
    state
        .global_settings
        .lock()
        .await
        .set_allow_lan_http(allow_lan_http)
        .await?;
    if allow_lan_http {
        warn!("Plain HTTP from other devices is now allowed: passwords and login tokens can cross the network unencrypted");
    } else {
        info!("Plain HTTP from other devices is now refused");
    }
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details: format!(
            "Plain HTTP from other devices {}",
            if allow_lan_http { "allowed" } else { "refused" }
        ),
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

pub async fn change_domain(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(new_domain): Json<String>,
) -> Result<(), Error> {
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core domain"),
        });
    }
    if new_domain.len() > 253 {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Domain too long"),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_domain(if new_domain.is_empty() {
            None
        } else {
            Some(new_domain)
        })
        .await?;
    Ok(())
}

/// Lets programs on this computer claim owner access through the core's data folder
/// (`crate::local_claim`), or stops them. Owner only.
pub async fn change_allow_local_claim(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("let programs on this computer claim owner access")?;
    if !enabled && !crate::remote_access::owner_has_password(&state).await {
        // the claim is then the only way to sign in as the owner
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!(
                "The owner has no password yet, so apps on this computer must be able to \
                 claim owner access. Set up the owner account first"
            ),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_allow_local_claim(enabled)
        .await?;
    let details = format!(
        "Claiming owner access from this computer turned {}",
        if enabled { "on" } else { "off" }
    );
    info!("{details}");
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details,
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

/// Turns the background check for newer versions of servers' modpacks on or off. Owner
/// only. "Check for an update" on a server still works while it is off.
pub async fn change_check_modpack_updates(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("turn modpack update checks on or off")?;
    state
        .global_settings
        .lock()
        .await
        .set_check_modpack_updates(enabled)
        .await?;
    let details = format!(
        "Checking for modpack updates turned {}",
        if enabled { "on" } else { "off" }
    );
    info!("{details}");
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details,
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

/// Shows or hides the listed instances at `GET /public/instances`, for Lodestone
/// Launcher. Owner only.
pub async fn change_list_instances_publicly(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("list servers publicly")?;
    state
        .global_settings
        .lock()
        .await
        .set_list_instances_publicly(enabled)
        .await?;
    state.public_listing.invalidate().await;
    info!(
        "Listing servers publicly turned {}",
        if enabled { "on" } else { "off" }
    );
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details: format!(
            "Listing servers publicly turned {}",
            if enabled { "on" } else { "off" }
        ),
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

/// Lets other devices use the core, or only this computer (see
/// [`crate::remote_access`]). Owner only. Turning it on needs an owner with a password
/// (`PUT /user/owner_account` sets one up and turns it on). Turning it off is only
/// allowed from this computer: from another device it would lock the requester out.
pub async fn change_allow_remote_access(
    axum::extract::State(state): axum::extract::State<AppState>,
    ClientIp(ip): ClientIp,
    requester: AuthedUser,
    Json(allowed): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("allow or refuse access from other devices")?;
    if allowed && !crate::remote_access::owner_has_password(&state).await {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!(
                "Set up the owner account with a password before allowing access from \
                 other devices"
            ),
        });
    }
    if !allowed && !ip.is_some_and(crate::tls::is_loopback) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "Turn off access from other devices in Lodestone Host, on the core's computer"
            ),
        });
    }
    crate::remote_access::set(&state, allowed, requester.caused_by()).await
}

pub fn get_global_settings_routes(state: AppState) -> Router {
    Router::new()
        .route("/global_settings", get(get_core_settings))
        .route("/global_settings/name", put(change_core_name))
        .route("/global_settings/safe_mode", put(change_core_safe_mode))
        .route(
            "/global_settings/code_execution",
            put(change_code_execution),
        )
        .route("/global_settings/domain", put(change_domain))
        .route(
            "/global_settings/allow_lan_http",
            put(change_allow_lan_http),
        )
        .route(
            "/global_settings/allow_local_claim",
            put(change_allow_local_claim),
        )
        .route(
            "/global_settings/list_instances_publicly",
            put(change_list_instances_publicly),
        )
        .route(
            "/global_settings/allow_remote_access",
            put(change_allow_remote_access),
        )
        .route(
            "/global_settings/check_modpack_updates",
            put(change_check_modpack_updates),
        )
        .with_state(state)
}
