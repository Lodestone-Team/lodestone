mod utils;
mod playit_secret;
mod errors;
use playit_agent_core::api::api::ApiError;
use tokio::task::JoinHandle;
use color_eyre::eyre::eyre;
use uuid::Uuid;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic::Ordering, Arc};
use axum::Json;
use playit_agent_core::api::{ PlayitApi, api::{AccountTunnelAllocation, ReqClaimExchange, PortType, ReqTunnelsList, TunnelType, ClaimExchangeError}};
use crate::error::{Error, ErrorKind};
use crate::AppState;
use crate::prelude::lodestone_path;
use serde::{Deserialize, Serialize};
use utils::*;
use playit_secret::*;

#[derive(Deserialize)]
pub struct PlayitTunnelParams {
    pub local_port: u16,
    pub port_type: PortType,
}

#[derive(Serialize, Deserialize)]
pub struct PlayitTunnelInfo {
    pub public_ip: String,
    pub public_port: u16,
    pub tunnel_id: Uuid,
}
pub struct TunnelHandle(Arc<AtomicBool>, JoinHandle<()>);

#[derive(Serialize, Deserialize)]
pub struct PlayitSignupData {
    pub url: String,
    pub claim_code: String,
}

#[derive(Serialize, Deserialize)]
pub enum SignupStatus {
	CodeNotFound,
	CodeExpired,
	UserRejected,
	NotAccepted,
	NotSetup,
    Completed,
}

impl From<ClaimExchangeError> for SignupStatus {
    fn from(error: ClaimExchangeError) -> Self {
        match error {
            ClaimExchangeError::CodeNotFound => SignupStatus::CodeNotFound,
            ClaimExchangeError::CodeExpired => SignupStatus::CodeExpired,
            ClaimExchangeError::UserRejected => SignupStatus::UserRejected,
            ClaimExchangeError::NotAccepted => SignupStatus::NotAccepted,
            ClaimExchangeError::NotSetup => SignupStatus::NotSetup,
        }
    }
}

pub async fn start_tunnel(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(params): Json<PlayitTunnelParams>,
) -> Result<Json<PlayitTunnelInfo>, Error> {
    let secret = state
        .playitgg_key
        .lock()
        .await
        .clone()
        .ok_or_else(|| {
            eyre!("Failed to get playitgg secret from state")
        })
        .unwrap();
    let tunnel_type: Option<TunnelType> = None;
    let name = Some(String::from(""));
    let api = make_client(String::from("https://api.playit.gg"), secret.clone());
    let port_count = 1;
    let exact = false;
    let ignore_name = true;

    let port_type = params.port_type;
    let port_num = params.local_port;

    let tunnels = api.tunnels_list(ReqTunnelsList { tunnel_id: None, agent_id: None })
        .await
        .map_err(|e| {
            eyre!("Failed to get tunnels from playitgg with error {:?}" , e)
        })
        .unwrap();
    let tunnel = find_tunnel(tunnels, name, port_type, port_count, tunnel_type, exact, ignore_name)
        .ok_or_else(|| {
            eyre!("Couldn't find tunnel.")
        })
        .unwrap();

    if let AccountTunnelAllocation::Allocated(allocated) = tunnel.clone().alloc {
        let tunnel_runner = get_tunnel(tunnel.clone(), allocated.clone(), secret, port_type, port_num)
            .await
            .map_err(|e| {
                eyre!("Failed to create tunnel object with error {:?}" , e)
            })
            .unwrap();

        let tunnel_future = TunnelHandle(
            tunnel_runner.keep_running(),
            tokio::spawn(async move {
                tunnel_runner.run().await;
            })
        );
        state.tunnels.lock().await.insert(allocated.id, tunnel_future);
        Ok(Json(PlayitTunnelInfo { public_ip: allocated.assigned_domain, public_port: allocated.port_start, tunnel_id: allocated.id }))
    } else {
        Err(Error{
                kind: ErrorKind::Internal,
                source: eyre!(
                    "Couldn't allocate tunnel."
                ),
        }) 
    }
}


pub async fn kill_tunnel(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(tunnel_info): Json<PlayitTunnelInfo>,
) -> Result<Json<()>, Error> {
    let tunnel_id = tunnel_info.tunnel_id;
    let tunnel = state
        .tunnels
        .lock()
        .await
        .remove(&tunnel_id)
        .ok_or_else(|| {
            eyre!("Couldn't find tunnel to kill.")
        })
        .unwrap();
    tunnel.0.store(false, Ordering::SeqCst);
    Ok(Json(()))
}


pub async fn generate_signup_link(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<PlayitSignupData>, Error> {
    let claim_code = claim_generate();
    let url = claim_url(&claim_code)
        .map_err(|e| {
            eyre!("Failed to generate signup url with error code {:?}" , e)
        })
        .unwrap();

    Ok(Json(PlayitSignupData { url, claim_code }))
}

pub async fn confirm_singup(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(signup_data): Json<PlayitSignupData>,
) -> Result<Json<SignupStatus>, Error> {
    let api = PlayitApi::create(API_BASE.to_string(), None);

    match api.claim_exchange(ReqClaimExchange { code: signup_data.claim_code.to_string() }).await {
        Ok(res) => {
            let mut secret_key_path = lodestone_path().clone();
            secret_key_path.push("playit.toml");
            let toml = toml::to_string(&res.secret_key).map_err(|e| {
                eyre!("Failed to serialize playit secret key with error {:?}" , e)
            })?;
            tokio::fs::write(secret_key_path, toml).await.map_err(|e| {
                eyre!("Failed to write playit secret key with error {:?}" , e)
            })?;
            Ok(Json(SignupStatus::Completed))
        }
        Err(ApiError::Fail(error)) => Ok(Json(error.into())),
        Err(error) =>  Err(Error{
                kind: ErrorKind::Internal,
                source: eyre!(
                    "Failed to confirm signup with error {:?}" , error
                ),
        })
    }
}