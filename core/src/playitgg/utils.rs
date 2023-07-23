/* 
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/


use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use crate::error::{Error, ErrorKind};
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::str::FromStr;
use std::time::Duration;

use color_eyre::eyre::eyre;
use playit_agent_common::Proto;
use playit_agent_core::tunnel::setup::SetupError;

use playit_agent_core::api::api::{AccountTunnel, AccountTunnelAllocation, AgentType, ApiError, ApiErrorNoFail, ApiResponseError, AssignedManagedCreate, ClaimSetupResponse, PortType, ReqClaimExchange, ReqClaimSetup, ReqTunnelsCreate, ReqTunnelsList, TunnelAllocated, TunnelOriginCreate, TunnelType, PlayitApiClient, AccountTunnels};
use playit_agent_core::api::http_client::{HttpClientError, HttpClient};
use playit_agent_core::api::ip_resource::IpResource;
use playit_agent_core::api::PlayitApi;
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};
use playit_agent_core_v09::agent_state::AgentState;
use playit_agent_core_v09::agent_updater::AgentUpdater;
use playit_agent_core_v09::api_client::ApiClient;
use playit_agent_core_v09::control_lookup::get_working_io;
use playit_agent_core_v09::ping_task::PingTask;
use playit_agent_core_v09::setup_config::{AgentConfigStatus, prepare_config};
use playit_agent_core_v09::tcp_client::{TcpClients, TcpConnection};
use playit_agent_core_v09::tunnel_api::TunnelApi;
use tokio::sync::RwLock;

use crate::prelude::lodestone_path;
use super::playit_secret::*;

#[derive(Clone)]
pub struct SimpleTunnel {
    pub pub_address: String,
    pub port_type: PortType,
    pub from_port: u16,
    pub to_port: u16,
    pub local_start_address: SocketAddr,
}


pub struct LocalLookup {
    pub data: Vec<SimpleTunnel>,
} 


impl AddressLookup for LocalLookup {
    type Value = SocketAddr;

    fn lookup(&self, _ip: IpAddr, port: u16, proto: PortType) -> Option<AddressValue<SocketAddr>> {
        for tunnel in &*self.data {
            if tunnel.port_type != proto && tunnel.port_type != PortType::Both {
                continue;
            }


            if tunnel.from_port <= port && port < tunnel.to_port {
                return Some(AddressValue {
                    value: tunnel.local_start_address,
                    from_port: tunnel.from_port,
                    to_port: tunnel.to_port,
                });
            }
        }

        Some(AddressValue {
            value: "127.0.0.1:25655".parse().unwrap(),
            from_port: port,
            to_port: port + 1,
        })
    }
}


pub async fn is_valid_secret_key(secret: String) -> bool {
    let api = PlayitApi::create(API_BASE.to_string(), Some(secret));
    api
        .tunnels_list(ReqTunnelsList {
            tunnel_id: None,
            agent_id: None,
        })
        .await
        .is_ok()
}

pub(super) async fn run_playit_cli(config_path: PathBuf) {
    let status = Arc::new(RwLock::new(AgentConfigStatus::default()));
    let config_path_str = match config_path.to_str() {
        Some(path) => path,
        None => {
            return Err(eyre!("Failed to convert config path to string")).unwrap();
        }
    };

    let prepare_config_task = {
        let status = status.clone();
        let config_path_str = config_path_str.to_string();
        tokio::spawn(async move {
            prepare_config(&config_path_str, &status).await
        })
    };

    let agent_config_res = loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = status.read().await;

        tokio::time::sleep(Duration::from_secs(5)).await;

        if prepare_config_task.is_finished() {
            break prepare_config_task.await.unwrap();
        }
    };

    let mut agent_config = match agent_config_res {
        Ok(config) => config.build(),
        Err(error) => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            return;
        }
    };

    if agent_config.api_refresh_rate.is_some() {
        let api_client = ApiClient::new(agent_config.api_url.clone(), Some(agent_config.secret_key.clone()));
        agent_config = match api_client.get_agent_config().await {
            Ok(updated) => agent_config.to_updated(updated.build()),
            Err(error) => {
                return;
            }
        };
    }
    
    let tunnel_io = match get_working_io(&agent_config.control_address).await {
        Some(v) => v,
        None => {
            return;
        }
    };

    let api_client = ApiClient::new(
        agent_config.api_url.clone(),
        Some(agent_config.secret_key.clone()),
    );

    let config_path_str = match config_path.to_str() {
        Some(path) => path,
        None => {
            return Err(eyre!("Failed to convert config path to string")).unwrap();
        }
    };

    let tunnel_api = TunnelApi::new(api_client, tunnel_io);
    let agent_updater = Arc::new(AgentUpdater::new(tunnel_api, AgentState {
        agent_config: RwLock::new(Arc::new(agent_config)),
        agent_config_save_path: Some(config_path_str.to_string()),
        ..AgentState::default()
    }));

    let agent_update_loop = {
        let agent_updater = agent_updater.clone();

        tokio::spawn(async move {
            loop {
                let wait = match agent_updater.update().await {
                    Ok(wait) => wait,
                    Err(error) => {
                        1000
                    }
                };

                tokio::time::sleep(Duration::from_millis(wait)).await;
            }
        })
    };

    let _ping_task_loop = {
        let ping_task = PingTask::new(agent_updater.state());
        tokio::spawn(ping_task.run())
    };

    let tcp_clients = Arc::new(TcpClients::default());

    /* process messages from tunnel server */
    let _message_process_task = {
        let agent_updater = agent_updater.clone();
        let tcp_clients = tcp_clients.clone();

        tokio::spawn(async move {
            loop {
                println!("poggers");
                match agent_updater.process_tunnel_feed().await {
                    Ok(Some(client)) => {
                        let agent_updater = agent_updater.clone();
                        let tcp_clients = tcp_clients.clone();

                        tokio::spawn(async move {
                            let (_bind_ip, local_addr) = {
                                let state = agent_updater.state();
                                let config = state.agent_config.read().await;

                                match config.find_local_addr(client.connect_addr, Proto::Tcp) {
                                    Some(v) => v,
                                    None => {
                                        tracing::info!(connect_addr = %client.connect_addr, "could not find tunnel for new connection");
                                        return;
                                    }
                                }
                            };

                            let conn_res = TcpConnection::spawn(
                                client,
                                local_addr,
                                tcp_clients,
                            ).await;

                            let connection = match conn_res {
                                Ok(connection) => connection,
                                Err(error) => {
                                    tracing::error!(?error, "failed to setup connection");
                                    return;
                                }
                            };

                            connection.wait().await;
                        });
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::error!(?error, "got error processing tunnel feed");
                    }
                }
            }
        })
    };

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        if agent_updater.state().authenticate_times.has_ack() {
            break;
        }
    }
}