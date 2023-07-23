mod utils;
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

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
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
    println!("{:?}", tunnels);
    let tunnel = find_tunnel(tunnels, name, port_type.clone().into(), port_count, tunnel_type, exact, ignore_name)
        .ok_or_else(|| {
            eyre!("Couldn't find tunnel.")
        })
        .unwrap();

    if let AccountTunnelAllocation::Allocated(allocated) = tunnel.clone().alloc {
        let tunnel_runner = get_tunnel(tunnel.clone(), allocated.clone(), secret, port_type.into(), port_num)
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
        state.tunnels.lock().await.insert(TunnelUuid(allocated.id.to_string()), tunnel_future);
        Ok(Json(PlayitTunnelInfo { public_ip: allocated.assigned_domain, public_port: allocated.port_start, tunnel_id: TunnelUuid(allocated.id.to_string()) }))
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

pub async fn get_playit_cli_url() -> Option<String> {
    match std::env::consts::OS {
        "windows" => Some(String::from("https://github.com/playit-cloud/playit-agent/releases/download/v0.9.3/playit-0.9.3-unsigned.exe")),
        "linux" => Some(String::from("https://github.com/playit-cloud/playit-agent/releases/download/v0.9.3/playit-0.9.3")),
        "macos" => Some(String::from("https://github.com/playit-cloud/playit-agent/releases/download/v0.9.3/playit-0.9.3-apple-m1")),
        _ => None,
    }
}

pub async fn start_cli(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<()>, Error> {
    let cli_name = get_playit_cli_url().await.ok_or_else(|| {
        eyre!("Failed to get playit cli url")
    })
    .unwrap()
    .split('/')
    .last()
    .ok_or_else(|| {
        eyre!("Internal error: Misformatted playit cli url")
    })
    .unwrap()
    .to_string();

    let cli_path = lodestone_path()
        .join("playitgg")
        .join(cli_name);
    let mut cli_start_command = Command::new(cli_path);
    println!("{:?}" , cli_start_command);

    match dont_spawn_terminal(&mut cli_start_command) 
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => {
            println!("Started playit cli");
            state.playit_cli_handle.lock().await.replace(child);
            Ok(Json(()))
        }
        Err(e) => {
            println!("{:?}", e);
            Err(Error{
                kind: ErrorKind::Internal,
                source: eyre!(
                    "Failed to start playit cli"
                ),
        })
    }

    }
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
                ClaimSetupResponse::WaitingForUserVisit => {
                    let msg = format!("Waiting for user to visit {}", url);
                    println!("{}", msg);
                }
                ClaimSetupResponse::WaitingForUser => {
                    println!("Waiting for user to approve");
                }
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
                toml.insert("ping_target_addresses".to_string(), Value::Array(vec![Value::String("23.133.216.1:5530".to_string()), Value::String("ping.ply.gg".to_string())]));
                toml.insert("last_update".to_string(), Value::String("control.playit.gg".to_string()));
                toml.insert("refresh_from_api".to_string(), Value::Boolean(true));
                toml.insert("api_refresh_rate".to_string(), Value::Integer(0));
                toml.insert("ping_interval".to_string(), Value::Integer(0));
                toml.insert("secret_key".to_string(), Value::String(res.secret_key.clone()));
                
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
    println!("generate_signup_link");
    Ok(ret_data)
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

            let mut toml = toml::map::Map::new();
            toml.insert("last_update".to_string(), Value::Integer(0));
            toml.insert("ping_target_addresses".to_string(), Value::Array(vec![Value::String("23.133.216.1:5530".to_string()), Value::String("ping.ply.gg".to_string())]));
            toml.insert("last_update".to_string(), Value::String("control.playit.gg".to_string()));
            toml.insert("refresh_from_api".to_string(), Value::Boolean(true));
            toml.insert("api_refresh_rate".to_string(), Value::Integer(0));
            toml.insert("ping_interval".to_string(), Value::Integer(0));
            toml.insert("secret_key".to_string(), Value::String(res.secret_key.clone()));
            
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
                        break;
                    }
                    
                    tokio::time::sleep(Duration::from_secs(3)).await;

                }
            });


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