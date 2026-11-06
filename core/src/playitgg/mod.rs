/*
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

pub mod helper;
pub mod utils;

mod playit_secret;
use std::future::Future;
use std::time::Duration;
use toml::Value;
mod errors;
use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event, EventInner, PlayitggRunnerEvent, PlayitggRunnerEventInner};
use crate::persist::write_atomic;
use crate::prelude::lodestone_path;
use crate::types::Snowflake;
use crate::AppState;
use axum::Json;
use color_eyre::eyre::eyre;
use helper::*;
use playit_agent_core::network::origin_lookup::OriginLookup;
use playit_api_client::{
    api::{
        AgentType, ApiError, ClaimSetupResponse, PortType as PlayitPortType, ReqClaimExchange,
        ReqClaimSetup, ReqTunnelsList,
    },
    PlayitApi,
};
use playit_secret::*;
use serde::{Deserialize, Serialize};
use std::sync::{atomic::Ordering, Arc};
use tokio::time::Instant;
use tracing::{error, info, warn};
use ts_rs::TS;
use utils::*;

/// Upper bound for a single request to the playit.gg API. The API client has no timeout
/// of its own, so a blackholed host would otherwise hang the caller forever.
pub(crate) const PLAYIT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a signup link stays valid before Lodestone stops polling for it.
const SIGNUP_DEADLINE: Duration = Duration::from_secs(15 * 60);
const SIGNUP_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct TunnelUuid(String);

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayitTunnelParams {
    pub local_port: u16,
    pub port_type: PortType,
}

#[derive(Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub enum PortType {
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "udp")]
    Udp,
    #[serde(rename = "both")]
    Both,
}

impl From<PortType> for PlayitPortType {
    fn from(port_type: PortType) -> Self {
        match port_type {
            PortType::Tcp => PlayitPortType::Tcp,
            PortType::Udp => PlayitPortType::Udp,
            PortType::Both => PlayitPortType::Both,
        }
    }
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayitTunnelInfo {
    pub local_ip: String,
    pub local_port: u16,
    pub tunnel_id: TunnelUuid,
    pub name: String,
    pub server_address: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct PlayitSignupData {
    pub url: String,
    pub claim_code: String,
}

/// Runs `fut` with [`PLAYIT_REQUEST_TIMEOUT`]. `what` describes the request for the error.
async fn with_timeout<T>(what: &str, fut: impl Future<Output = T>) -> Result<T, Error> {
    tokio::time::timeout(PLAYIT_REQUEST_TIMEOUT, fut)
        .await
        .map_err(|_| eyre!("Timed out while {what}").into())
}

/// Fetches the agent's run data and builds a tunnel runner for `secret_key`.
async fn create_runner(secret_key: String) -> Result<TunnelRunner, Error> {
    let api = PlayitApi::create(API_BASE.to_string(), Some(secret_key.clone()));
    let data = with_timeout("getting run data from playit.gg", api.agents_rundata())
        .await?
        .map_err(|e| eyre!("Failed to get run data from playit.gg: {e:?}"))?;
    let lookup = Arc::new(OriginLookup::default());
    lookup.update_from_run_data(&data).await;
    with_timeout(
        "starting the playit.gg agent",
        TunnelRunner::new(API_BASE.to_string(), secret_key, lookup),
    )
    .await?
    .map_err(|e| eyre!("Failed to create the playit.gg runner: {e:?}").into())
}

pub async fn start_cli(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<()>, Error> {
    // Hold the lock from the "already running" check until the new handle is stored, so
    // two concurrent requests cannot start two agents (the first of which could then never
    // be stopped).
    let mut keep_running_slot = state.playit_keep_running.lock().await;
    if let Some(keep_running) = keep_running_slot.as_ref() {
        if keep_running.load(Ordering::SeqCst) {
            return Ok(Json(()));
        }
    }

    let playitgg_key = state
        .playitgg_key
        .lock()
        .await
        .clone()
        .ok_or_else(|| eyre!("No playitgg key found"))?;
    let runner = create_runner(playitgg_key).await?;
    keep_running_slot.replace(runner.keep_running());
    drop(keep_running_slot);

    let event_broadcaster = state.event_broadcaster.clone();
    tokio::spawn(async move {
        runner.run(event_broadcaster, true).await;
    });

    Ok(Json(()))
}

pub async fn stop_cli(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<()>, Error> {
    if let Some(keep_running) = state.playit_keep_running.lock().await.clone() {
        if keep_running.load(Ordering::SeqCst) {
            state.event_broadcaster.send(Event {
                event_inner: EventInner::PlayitggRunnerEvent(PlayitggRunnerEvent {
                    playitgg_runner_event_inner: PlayitggRunnerEventInner::RunnerStopped,
                }),
                snowflake: Snowflake::default(),
                details: "Stopped".to_string(),
                caused_by: CausedBy::System,
            });
            keep_running.store(false, Ordering::SeqCst);
        }
    }

    Ok(Json(()))
}

pub async fn cli_is_running(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<bool>, Error> {
    if let Some(keep_running) = state.playit_keep_running.lock().await.clone() {
        Ok(Json(keep_running.load(Ordering::SeqCst)))
    } else {
        Ok(Json(false))
    }
}

/// Clears the "signup in progress" slot when the signup task ends, even if it panics.
struct SignupSlotGuard(Arc<std::sync::Mutex<Option<PlayitSignupData>>>);

impl Drop for SignupSlotGuard {
    fn drop(&mut self) {
        match self.0.lock() {
            Ok(mut slot) => *slot = None,
            Err(poisoned) => *poisoned.into_inner() = None,
        }
    }
}

/// Starts a playit.gg signup and returns the link the user must visit.
///
/// Only one signup runs at a time: while one is in progress, this returns the same link.
/// The background task gives up after [`SIGNUP_DEADLINE`] and reports the outcome as a
/// progression event.
pub async fn generate_signup_link(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<PlayitSignupData>, Error> {
    let signup_data = {
        let mut slot = state
            .playit_signup
            .lock()
            .map_err(|_| eyre!("playit.gg signup state is poisoned"))?;
        if let Some(existing) = slot.as_ref() {
            return Ok(Json(existing.clone()));
        }
        let claim_code = claim_generate();
        let url = claim_url(&claim_code)
            .map_err(|e| eyre!("Failed to generate signup url with error code {:?}", e))?;
        let signup_data = PlayitSignupData { url, claim_code };
        *slot = Some(signup_data.clone());
        signup_data
    };

    let guard = SignupSlotGuard(state.playit_signup.clone());
    let claim_code = signup_data.claim_code.clone();
    tokio::spawn(async move {
        let _guard = guard;
        let (start_event, event_id) = Event::new_progression_event_start(
            "Setting up playit.gg",
            None,
            None,
            CausedBy::System,
        );
        state.event_broadcaster.send(start_event);
        let result = run_signup(&state, claim_code).await;
        let (success, message) = match result {
            Ok(()) => {
                info!("playit.gg signup finished");
                (true, "playit.gg is set up".to_string())
            }
            Err(e) => {
                error!("playit.gg signup failed: {}", e.source);
                (false, format!("playit.gg setup failed: {}", e.source))
            }
        };
        state
            .event_broadcaster
            .send(Event::new_progression_event_end(
                event_id,
                success,
                Some(message),
                None,
            ));
    });

    Ok(Json(signup_data))
}

/// Waits for the user to accept the claim, stores the secret, and keeps an agent running
/// until playit.gg reports a tunnel, as the playit CLI does on first setup.
async fn run_signup(state: &AppState, claim_code: String) -> Result<(), Error> {
    let deadline = Instant::now() + SIGNUP_DEADLINE;
    let expired = || -> Error {
        eyre!(
            "the signup link expired after {} minutes",
            SIGNUP_DEADLINE.as_secs() / 60
        )
        .into()
    };
    let api = PlayitApi::create(API_BASE.to_string(), None);

    loop {
        if Instant::now() >= deadline {
            return Err(expired());
        }
        let setup = with_timeout(
            "checking the playit.gg signup",
            api.claim_setup(ReqClaimSetup {
                code: claim_code.clone(),
                agent_type: AgentType::Assignable,
                version: format!("playit-cli {}", "1.0.0-rc3"),
            }),
        )
        .await;
        match setup {
            Ok(Ok(ClaimSetupResponse::UserAccepted)) => {
                info!("playit.gg signup accepted, exchanging code for secret");
                break;
            }
            Ok(Ok(ClaimSetupResponse::UserRejected)) => {
                return Err(eyre!("the signup was rejected on playit.gg").into());
            }
            Ok(Ok(_)) => {}
            // InvalidCode, CodeExpired, ...: retrying cannot help
            Ok(Err(ApiError::Fail(fail))) => {
                return Err(eyre!("playit.gg refused the signup code: {fail:?}").into());
            }
            Ok(Err(e)) => warn!("playit.gg signup check failed, retrying: {e:?}"),
            Err(e) => warn!("{}, retrying", e.source),
        }
        tokio::time::sleep(SIGNUP_POLL_INTERVAL).await;
    }

    let secret_key = with_timeout(
        "exchanging the playit.gg claim code",
        api.claim_exchange(ReqClaimExchange { code: claim_code }),
    )
    .await?
    .map_err(|e| match e {
        ApiError::Fail(fail) => eyre!("playit.gg refused the claim exchange: {fail:?}"),
        e => eyre!("Failed to exchange the playit.gg claim code: {e:?}"),
    })?
    .secret_key;

    write_playit_config(&secret_key).await?;

    let runner = create_runner(secret_key.clone()).await?;
    let signal = runner.keep_running();
    tokio::spawn(runner.run(state.event_broadcaster.clone(), false));

    let api = PlayitApi::create(API_BASE.to_string(), Some(secret_key.clone()));
    let tunnel_result = loop {
        if Instant::now() >= deadline {
            break Err(expired());
        }
        match with_timeout("getting tunnels from playit.gg", api.agents_rundata()).await {
            Ok(Ok(data)) if !data.tunnels.is_empty() => break Ok(()),
            Ok(Ok(_)) => {}
            Ok(Err(e)) => warn!("Failed to get tunnels from playit.gg, retrying: {e:?}"),
            Err(e) => warn!("{}, retrying", e.source),
        }
        tokio::time::sleep(SIGNUP_POLL_INTERVAL).await;
    };
    // the setup agent is only needed until the first tunnel exists
    signal.store(false, Ordering::SeqCst);
    tunnel_result?;

    state.playitgg_key.lock().await.replace(secret_key);
    Ok(())
}

/// Writes `playit.toml` with the agent secret. The file is only readable by its owner.
async fn write_playit_config(secret_key: &str) -> Result<(), Error> {
    let mut toml = toml::map::Map::new();
    toml.insert("last_update".to_string(), Value::Integer(0));
    toml.insert(
        "api_url".to_string(),
        Value::String("https://api.playit.cloud/agent".to_string()),
    );
    toml.insert(
        "ping_target_addresses".to_string(),
        Value::Array(vec![
            Value::String("23.133.216.1:5530".to_string()),
            Value::String("ping.ply.gg".to_string()),
        ]),
    );
    toml.insert(
        "control_address".to_string(),
        Value::String("control.playit.gg".to_string()),
    );
    toml.insert("refresh_from_api".to_string(), Value::Boolean(true));
    toml.insert("api_refresh_rate".to_string(), Value::Integer(5000));
    toml.insert("ping_interval".to_string(), Value::Integer(5000));
    toml.insert(
        "secret_key".to_string(),
        Value::String(secret_key.to_string()),
    );
    toml.insert("mappings".to_string(), Value::Array(vec![]));

    write_atomic(
        lodestone_path().join("playit.toml"),
        toml.to_string(),
        Some(0o600),
    )
    .await
    .map_err(|e| eyre!("Failed to write the playit secret file: {}", e.source).into())
}

pub async fn verify_key(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<bool>, Error> {
    let secret_key = match state.playitgg_key.lock().await.clone() {
        Some(key) => key,
        None => return Ok(Json(false)),
    };
    Ok(Json(is_valid_secret_key(secret_key).await))
}

#[derive(Deserialize)]
struct RawTunnelList {
    tunnels: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawData<T> {
    data: T,
}

#[derive(Deserialize)]
struct RawAlloc {
    assigned_domain: String,
    port_start: Option<i64>,
}

#[derive(Deserialize)]
struct RawOrigin {
    local_ip: String,
    local_port: i64,
}

#[derive(Deserialize)]
struct RawTunnel {
    id: String,
    name: String,
    active: bool,
    alloc: RawData<RawAlloc>,
    origin: RawData<RawOrigin>,
}

impl TryFrom<RawTunnel> for PlayitTunnelInfo {
    type Error = color_eyre::Report;

    fn try_from(tunnel: RawTunnel) -> Result<Self, Self::Error> {
        let local_port = u16::try_from(tunnel.origin.data.local_port)
            .map_err(|_| eyre!("invalid local port {}", tunnel.origin.data.local_port))?;
        let domain = tunnel.alloc.data.assigned_domain;
        let server_address = match tunnel.alloc.data.port_start {
            Some(port) => format!("{domain}:{port}"),
            None => domain,
        };
        Ok(PlayitTunnelInfo {
            local_ip: tunnel.origin.data.local_ip,
            local_port,
            tunnel_id: TunnelUuid(tunnel.id),
            name: tunnel.name,
            server_address,
            active: tunnel.active,
        })
    }
}

/// Parses the `/tunnels/list` response. Tunnels that are not fully set up yet (no
/// allocation) or that have unexpected data are skipped with a warning.
fn parse_tunnel_list(response: serde_json::Value) -> Result<Vec<PlayitTunnelInfo>, Error> {
    let list: RawTunnelList = serde_json::from_value(response).map_err(|e| Error {
        kind: ErrorKind::External,
        source: eyre!("Got malformed response from Playit: {e}"),
    })?;
    Ok(list
        .tunnels
        .into_iter()
        .filter_map(|value| {
            let parsed = serde_json::from_value::<RawTunnel>(value)
                .map_err(color_eyre::Report::from)
                .and_then(PlayitTunnelInfo::try_from);
            match parsed {
                Ok(tunnel) => Some(tunnel),
                Err(e) => {
                    warn!("Skipping playit.gg tunnel with unexpected data: {e}");
                    None
                }
            }
        })
        .collect())
}

pub async fn get_tunnels(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<PlayitTunnelInfo>>, Error> {
    let secret = state
        .playitgg_key
        .lock()
        .await
        .clone()
        .ok_or_else(|| Error {
            kind: ErrorKind::Internal,
            source: eyre!("Couldn't find Playit key"),
        })?;
    let api = make_client(API_BASE.to_string(), secret);
    let response = with_timeout(
        "listing playit.gg tunnels",
        api.tunnels_list_json(ReqTunnelsList {
            tunnel_id: None,
            agent_id: None,
        }),
    )
    .await?
    .map_err(|e| Error {
        kind: ErrorKind::External,
        source: eyre!("Couldn't connect to Playit: {e:?}"),
    })?;
    parse_tunnel_list(response).map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tunnel(alloc: serde_json::Value, local_port: i64) -> serde_json::Value {
        json!({
            "id": "t1",
            "name": "minecraft",
            "active": true,
            "alloc": alloc,
            "origin": { "type": "agent", "data": { "local_ip": "127.0.0.1", "local_port": local_port } }
        })
    }

    #[test]
    fn parses_tunnels_without_panicking() {
        let response = json!({
            "tunnels": [
                tunnel(json!({ "status": "allocated", "data": { "assigned_domain": "a.ply.gg", "port_start": 1234 } }), 25565),
                // no port_start: used to panic
                tunnel(json!({ "status": "allocated", "data": { "assigned_domain": "b.ply.gg" } }), 25566),
                // pending allocation: skipped
                tunnel(json!({ "status": "pending" }), 25567),
                // port does not fit in u16: skipped instead of truncated
                tunnel(json!({ "status": "allocated", "data": { "assigned_domain": "c.ply.gg", "port_start": 1 } }), 70000),
            ]
        });
        let tunnels = parse_tunnel_list(response).unwrap();
        assert_eq!(tunnels.len(), 2);
        assert_eq!(tunnels[0].server_address, "a.ply.gg:1234");
        assert_eq!(tunnels[0].local_port, 25565);
        assert_eq!(tunnels[1].server_address, "b.ply.gg");
    }

    #[test]
    fn malformed_tunnel_list_is_an_error() {
        assert!(parse_tunnel_list(json!({ "nope": [] })).is_err());
        assert!(parse_tunnel_list(json!("text")).is_err());
    }

    #[test]
    fn signup_guard_clears_slot() {
        let slot = Arc::new(std::sync::Mutex::new(Some(PlayitSignupData {
            url: "u".to_string(),
            claim_code: "c".to_string(),
        })));
        drop(SignupSlotGuard(slot.clone()));
        assert!(slot.lock().unwrap().is_none());
    }
}
