/*
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{CausedBy, Event, EventInner, PlayitggRunnerEvent, PlayitggRunnerEventInner};
use crate::types::Snowflake;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::time::Duration;

use color_eyre::eyre::eyre;
use playit_agent_common::Proto;
use playit_agent_core::tunnel::setup::SetupError;

use super::tcp_client::{TcpClients, TcpConnection};
use playit_agent_core::api::api::{
    AgentType, ApiError, ApiErrorNoFail, ApiResponseError, AssignedManagedCreate,
    ClaimSetupResponse, PlayitApiClient, PortType, ReqClaimExchange, ReqClaimSetup,
    ReqTunnelsCreate, ReqTunnelsList, TunnelOriginCreate, TunnelType,
};
use playit_agent_core::api::http_client::{HttpClient, HttpClientError};
use playit_agent_core::api::ip_resource::IpResource;
use playit_agent_core::api::PlayitApi;
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};

use tokio::sync::RwLock;

use super::playit_secret::*;

#[derive(Clone)]
pub struct SimpleTunnel {
    pub pub_address: String,
    pub port_type: PortType,
    pub from_port: u16,
    pub to_port: u16,
    pub local_start_address: SocketAddr,
}

pub async fn is_valid_secret_key(secret: String) -> bool {
    let api = PlayitApi::create(API_BASE.to_string(), Some(secret));
    api.agents_rundata().await.is_ok()
}

pub fn make_client(api_base: String, secret: String) -> PlayitApiClient<HttpClient> {
    PlayitApi::create(api_base, Some(secret))
}
