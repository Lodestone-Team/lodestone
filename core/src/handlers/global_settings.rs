use super::util::{AuthedUser, ClientIp};
use axum::{
    routing::{get, put},
    Json, Router,
};
use color_eyre::eyre::eyre;
use tracing::{info, warn};

use crate::{
    error::ErrorKind,
    events::{Event, EventInner},
    types::Snowflake,
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

/// Turns code execution on or off. Turning it off stops every running macro, including the
/// ones that run atom instances, and tells connected clients to fetch the settings again.
/// Turning it on starts nothing.
pub async fn change_code_execution(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("turn code execution on or off")?;
    // Held while the running macros are stopped, so turning it back on waits until they are.
    let mut global_settings = state.global_settings.lock().await;
    global_settings.set_code_execution_enabled(enabled).await?;
    if !enabled {
        info!("Code execution turned off, stopping all macros");
        // Every macro spawned before the switch went off is in the process table, see
        // `MacroExecutor::spawn`. Killing them sends their Stopped events.
        state.macro_executor.shutdown_all();
    } else {
        info!("Code execution turned on");
    }
    drop(global_settings);
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
        .with_state(state)
}
