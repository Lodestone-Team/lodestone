pub mod utils;
use std::process::Stdio;
mod playit_secret;
use tokio::process::Command;
use toml::{Table, Value};
use std::time::Duration;
mod errors;
use playit_agent_core::api::api::ApiError;
use tokio::task::JoinHandle;
use color_eyre::eyre::eyre;
use uuid::Uuid;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic::Ordering, Arc};
use axum::Json;
use playit_agent_core::api::{ PlayitApi, api::{AccountTunnelAllocation, ClaimSetupResponse, ReqClaimSetup, AgentType, ReqClaimExchange, PortType as PlayitPortType, ReqTunnelsList, TunnelType, ClaimExchangeError}};
use crate::error::{Error, ErrorKind};
use crate::AppState;
use crate::prelude::lodestone_path;
use crate::util::dont_spawn_terminal;
use serde::{Deserialize, Serialize};
use utils::*;
use playit_secret::*;
use tokio::io::AsyncWriteExt; 
use playit_agent_core::tunnel_runner::TunnelRunner;
use ts_rs::TS;

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

impl From <PortType> for PlayitPortType {
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
    pub public_ip: String,
    pub public_port: u16,
    pub tunnel_id: TunnelUuid,
}
pub struct TunnelHandle(Arc<AtomicBool>, JoinHandle<()>);

#[derive(Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct PlayitSignupData {
    pub url: String,
    pub claim_code: String,
}

pub async fn start_cli(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<()>, Error> {
    if state.playit_running.load(Ordering::SeqCst) {
        return Ok(Json(()));
    }

    state.playit_running.store(true, Ordering::SeqCst);
    let config_path = lodestone_path().join("playit.toml");
    tokio::spawn(async move {
        run_playit_cli(config_path, state.playit_running).await;       
    });

    Ok(Json(()))
}


pub async fn stop_cli(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<()>, Error> {
    state.playit_running.store(false, Ordering::SeqCst);
    Ok(Json(()))
}


pub async fn generate_signup_link(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<PlayitSignupData>, Error> {
    let api = PlayitApi::create(API_BASE.to_string(), None);

    let claim_code = claim_generate();
    let url = claim_url(&claim_code)
        .map_err(|e| {
            eyre!("Failed to generate signup url with error code {:?}" , e)
        })
        .unwrap();
    let signup_data = Json(PlayitSignupData { url: url.clone(), claim_code: claim_code.clone() });
    let ret_data = signup_data.clone();
    
    tokio::spawn(async move {
        loop {
            let setup = api.claim_setup(ReqClaimSetup {
                code: claim_code.to_string(),
                agent_type: AgentType::Assignable,
                version: format!("playit-cli {}", "1.0.0-rc3"),
            }).await
            .map_err(|e| {
                eyre!("Failed to claim setup {:?}" , e)
            })
            .unwrap();

            match setup {
                ClaimSetupResponse::UserAccepted => {
                    println!("User accepted, exchanging code for secret");
                    break;
                }
                ClaimSetupResponse::UserRejected => {
                    println!("User rejected");
                    return Err(Error{
                        kind: ErrorKind::Internal,
                        source: eyre!(
                            "Failed to confirm signup with error {:?}" , setup
                        ),
                    });
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        };

        let api = PlayitApi::create(API_BASE.to_string(), None);
        match api.claim_exchange(ReqClaimExchange { code: signup_data.claim_code.to_string() }).await {
            Ok(res) => {
                let mut secret_key_path = lodestone_path().clone();
                secret_key_path.push("playit.toml");

                let mut toml = toml::map::Map::new();
                toml.insert("last_update".to_string(), Value::Integer(0));
                toml.insert("api_url".to_string(), Value::String("https://api.playit.cloud/agent".to_string()));
                toml.insert("ping_target_addresses".to_string(), Value::Array(vec![Value::String("23.133.216.1:5530".to_string()), Value::String("ping.ply.gg".to_string())]));
                toml.insert("control_address".to_string(), Value::String("control.playit.gg".to_string()));
                toml.insert("refresh_from_api".to_string(), Value::Boolean(true));
                toml.insert("api_refresh_rate".to_string(), Value::Integer(5000));
                toml.insert("ping_interval".to_string(), Value::Integer(5000));
                toml.insert("secret_key".to_string(), Value::String(res.secret_key.clone()));
                toml.insert("mappings".to_string(), Value::Array(vec![]));
                
                let mut file = tokio::fs::File::create(secret_key_path).await.map_err(|e| {
                    eyre!("Failed to create playit secret file with error {:?}" , e)
                })?;
                file.write_all(toml.to_string().as_bytes()).await.map_err(|e| {
                    eyre!("Failed to write playit secret key with error {:?}" , e)
                })?;
                
                let api = PlayitApi::create(API_BASE.to_string(), Some(res.secret_key.clone()));
                
                let lookup = Arc::new(LocalLookup {
                    data: vec![],
                });
            
                let runner = TunnelRunner::new(res.secret_key.clone(), lookup.clone())
                    .await
                    .map_err(|e| {
                        eyre!("Failed to create tunnel object with error {:?}" , e)
                    })
                    .unwrap();

                tokio::spawn(async move {
                    let signal = runner.keep_running();
                    tokio::spawn(runner.run());
                    loop {
                        let tunnels = api.tunnels_list(ReqTunnelsList { tunnel_id: None, agent_id: None })
                            .await
                            .map_err(|e| {
                                eyre!("Failed to get tunnels from playitgg with error {:?}" , e)
                            })
                            .unwrap();

                        if !tunnels.tunnels.is_empty() {
                            signal.store(false, Ordering::SeqCst);
                            state.playitgg_key.lock().await.replace(res.secret_key.clone());
                            break;
                        }
                        
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                });
                Ok(())
            }
            Err(ApiError::Fail(error)) => Err(Error{
                    kind: ErrorKind::Internal,
                    source: eyre!(
                        "Api error: {:?}" , error
                    ),
            }),
            Err(error) =>  Err(Error{
                    kind: ErrorKind::Internal,
                    source: eyre!(
                        "Failed to confirm signup with error {:?}" , error
                    ),
            })
        }
    });

    Ok(ret_data)
}



pub async fn confirm_singup(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<bool>, Error> {
    let secret_key = match state.playitgg_key.lock().await.clone() {
        Some(key) => key,
        None => {
            return Err(Error{
                kind: ErrorKind::Internal,
                source: eyre!(
                    "Failed to parse secret key"
                ),
            })
        }
    };
    Ok(Json(is_valid_secret_key(secret_key).await))
}

