mod utils;
mod playit_secret;
mod errors;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{atomic::Ordering, Arc, Mutex};
use std::str::FromStr;
use std::time::Duration;

use playit_agent_core::tunnel::setup::SetupError;
use axum::{routing::get, Json, Router};
use playit_agent_core::api::api::{AccountTunnel, AccountTunnelAllocation, AgentType, ApiError, ApiErrorNoFail, ApiResponseError, AssignedManagedCreate, ClaimSetupResponse, PortType, ReqClaimExchange, ReqClaimSetup, ReqTunnelsCreate, ReqTunnelsList, TunnelAllocated, TunnelOriginCreate, TunnelType};
use playit_agent_core::api::http_client::HttpClientError;
use playit_agent_core::api::ip_resource::IpResource;
use playit_agent_core::api::PlayitApi;
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};
use crate::AppState;

use utils::*;
use errors::*;

pub async fn start_tunnel(axum::extract::State(state): axum::extract::State<AppState>) -> Json<()> {
    let secret = state.playitgg_key.lock().await.clone().unwrap();
    let tunnel_type: Option<TunnelType> = None;
    let name = Some(String::from("neat"));
    let port_type = PortType::Tcp;
    
    tokio::time::sleep(Duration::from_secs(1)).await;

    let api = make_client(String::from("https://api.playit.gg"), secret.clone());

    let port_count = 1;
    let port_num = 25655;
    let exact = false;
    let ignore_name = false;


    let tunnels = api.tunnels_list(ReqTunnelsList { tunnel_id: None, agent_id: None }).await.unwrap();
    let tunnel = find_tunnel(tunnels, name, port_type, port_count, tunnel_type, exact, ignore_name);

    if let AccountTunnelAllocation::Allocated(allocated) = tunnel.clone().unwrap().alloc {
        run_tunnel(tunnel.clone().unwrap(), allocated, secret, port_type, port_num).await;
    }
    Json(())
}