/* 
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/


use std::collections::{HashMap, HashSet};
use crate::error::{Error, ErrorKind};
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