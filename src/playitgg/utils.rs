/* 
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/


use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::str::FromStr;
use std::time::Duration;

use playit_agent_core::tunnel::setup::SetupError;

use playit_agent_core::api::api::{AccountTunnel, AccountTunnelAllocation, AgentType, ApiError, ApiErrorNoFail, ApiResponseError, AssignedManagedCreate, ClaimSetupResponse, PortType, ReqClaimExchange, ReqClaimSetup, ReqTunnelsCreate, ReqTunnelsList, TunnelAllocated, TunnelOriginCreate, TunnelType, PlayitApiClient, AccountTunnels};
use playit_agent_core::api::http_client::{HttpClientError, HttpClient};
use playit_agent_core::api::ip_resource::IpResource;
use playit_agent_core::api::PlayitApi;
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};

use playit_agent_core::tunnel_runner::TunnelRunner;
use playit_agent_core::utils::now_milli;

use tokio::sync::RwLock;

use super::playit_secret::*;
use super::errors::CliError;

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

    fn lookup(&self, ip: IpAddr, port: u16, proto: PortType) -> Option<AddressValue<SocketAddr>> {
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
            value: "127.0.0.1".parse().unwrap(),
            from_port: port,
            to_port: port + 1,
        })
    }
}


pub fn make_client(api_base:String, secret: String) -> PlayitApiClient<HttpClient>{
    PlayitApi::create(api_base, Some(secret))
}

pub fn find_tunnel(tunnels: AccountTunnels, name: Option<String>, port_type: PortType, port_count: u16, tunnel_type: Option<TunnelType>, exact: bool, ignore_name: bool) -> Option<AccountTunnel> {
    let mut options = Vec::new();
    for tunnel in tunnels.tunnels {
        let tunnel_port_count = match &tunnel.alloc {
            AccountTunnelAllocation::Allocated(alloc) => alloc.port_end - alloc.port_start,
            _ => continue,
        };

        if exact {
            if (ignore_name || tunnel.name.eq(&name)) && tunnel.port_type == port_type && port_count == tunnel_port_count && tunnel.tunnel_type == tunnel_type {
                options.push(tunnel);
            } else {
                continue;
            }
        } else {
            if (tunnel.port_type == PortType::Both || tunnel.port_type == port_type) && port_count <= tunnel_port_count && tunnel.tunnel_type == tunnel_type {
                options.push(tunnel);
            }
        }
    }

    /* rank options by how much they match */
    options.sort_by_key(|option| {
        let mut points = 0;

        if ignore_name {
            if name.is_some() && option.name.eq(&name) {
                points += 1;
            }
        } else {
            if option.name.eq(&name) {
                points += 10;
            }
        }

        if option.port_type == port_type {
            points += 200;
        }

        if port_count == option.port_count {
            points += 100;
        } else {
            points += ((port_count as i32) - (option.port_count as i32)) * 10;
        }

        points += match option.alloc {
            AccountTunnelAllocation::Pending => -10,
            AccountTunnelAllocation::Disabled => -40,
            AccountTunnelAllocation::Allocated(_) => 0,
        };

        points
    });

    let mut tunnel: Option<AccountTunnel> = None;
    if let Some(found_tunnel) = options.pop() {
        Some(found_tunnel)
    } else {
        None
    }
}

pub async fn run_tunnel(tunnel: AccountTunnel, allocated: TunnelAllocated, secret: String, port_type: PortType, port_num: u16) -> Result<TunnelRunner<LocalLookup>, SetupError> {
    let simple_tunnel = SimpleTunnel {
        pub_address: allocated.ip_hostname,
        port_type,
        from_port: port_num,
        to_port: port_num + 1,
        local_start_address: "127.0.0.1:5000".parse().unwrap(),
    };
    let local_lookup = LocalLookup {
        data: vec!(simple_tunnel.clone()),
    };
    TunnelRunner::new(secret.clone(), Arc::new(local_lookup)).await
}

pub async fn verify_user(mut secret: PlayitSecret) -> Result<String, CliError> {
    secret
        .with_default_path()
        .ensure_valid()
        .await?
        .get_or_setup()
        .await
}