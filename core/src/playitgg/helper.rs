use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::Instrument;

use playit_agent_core::api::api::{AgentTunnel, PortType};
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};
use playit_agent_core::network::lan_address::LanAddress;
use playit_agent_core::network::tcp_clients::TcpClients;
use playit_agent_core::network::udp_clients::UdpClients;
use playit_agent_core::tunnel::setup::SetupError;
use playit_agent_core::tunnel::simple_tunnel::SimpleTunnel;
use playit_agent_core::tunnel::udp_tunnel::UdpTunnelRx;
use playit_agent_core::utils::now_milli;

use crate::event_broadcaster::{self, EventBroadcaster};
use crate::events::{CausedBy, Event, EventInner, PlayitggRunnerEvent, PlayitggRunnerEventInner};
use crate::types::Snowflake;

pub struct TunnelRunner<L: AddressLookup> {
    lookup: Arc<L>,
    tunnel: SimpleTunnel,
    udp_clients: UdpClients<Arc<L>>,
    tcp_clients: TcpClients,
    keep_running: Arc<AtomicBool>,
}

impl<L: AddressLookup + Sync + Send> TunnelRunner<L>
where
    L::Value: Into<SocketAddr>,
{
    pub async fn new(
        api_url: String,
        secret_key: String,
        lookup: Arc<L>,
    ) -> Result<Self, SetupError> {
        let tunnel = SimpleTunnel::setup(api_url, secret_key).await?;
        let udp_clients = UdpClients::new(tunnel.udp_tunnel(), lookup.clone());

        Ok(TunnelRunner {
            lookup,
            tunnel,
            udp_clients,
            tcp_clients: TcpClients::new(),
            keep_running: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn set_use_special_lan(&mut self, set_use: bool) {
        self.tcp_clients.use_special_lan = set_use;
        self.udp_clients.use_special_lan = set_use;
    }

    pub fn keep_running(&self) -> Arc<AtomicBool> {
        self.keep_running.clone()
    }

    pub async fn run(self, event_broadcaster: EventBroadcaster) {
        event_broadcaster.clone().send(Event {
            event_inner: EventInner::PlayitggRunnerEvent(PlayitggRunnerEvent {
                playitgg_runner_event_inner: PlayitggRunnerEventInner::RunnerLoading,
            }),
            snowflake: Snowflake::default(),
            details: "Starting runner".to_string(),
            caused_by: CausedBy::System,
        });

        let mut tunnel = self.tunnel;
        let udp = tunnel.udp_tunnel();

        let tunnel_run = self.keep_running.clone();

        let tunnel_task = tokio::spawn(async move {
            let mut last_control_update = now_milli();

            while tunnel_run.load(Ordering::SeqCst) {
                /* refresh control address every half minute */
                {
                    let now = now_milli();
                    if 30_000 < now_milli() - last_control_update {
                        last_control_update = now;

                        if let Err(error) = tunnel.reload_control_addr().await {
                            tracing::error!(?error, "failed to reload_control_addr");
                        }
                    }
                }

                if let Some(new_client) = tunnel.update().await {
                    let clients = self.tcp_clients.clone();
                    let span = tracing::info_span!("tcp client", ?new_client);

                    let local_addr = match self.lookup.lookup(
                        new_client.connect_addr.ip(),
                        new_client.connect_addr.port(),
                        PortType::Tcp,
                    ) {
                        Some(found) => {
                            let addr = found.value.into();
                            let port_offset = new_client.connect_addr.port() - found.from_port;
                            SocketAddr::new(addr.ip(), port_offset + addr.port())
                        }
                        None => {
                            tracing::info!("could not find local address for connection");
                            continue;
                        }
                    };
                    
                    let pipe_run = tunnel_run.clone();
                    tokio::spawn(
                        async move {
                            let peer_addr = new_client.peer_addr;

                            let tunnel_conn = match clients.connect(new_client.clone()).await {
                                Ok(Some(client)) => client,
                                Ok(None) => return,
                                Err(error) => {
                                    tracing::error!(?error, "failed to accept new client");
                                    return;
                                }
                            };

                            tracing::info!("connected to TCP tunnel");

                            let local_conn = match LanAddress::tcp_socket(
                                self.tcp_clients.use_special_lan,
                                peer_addr,
                                local_addr,
                            )
                            .await
                            {
                                Ok(v) => v,
                                Err(error) => {
                                    tracing::error!(?error, "failed to connect to local server");
                                    return;
                                }
                            };

                            let (tunnel_read, tunnel_write) = tunnel_conn.into_split();
                            let (local_read, local_write) = local_conn.into_split();

                            tokio::spawn(pipe(tunnel_read, local_write, pipe_run.clone()));
                            tokio::spawn(pipe(local_read, tunnel_write, pipe_run.clone()));
                        }
                        .instrument(span),
                    );
                }
            }
            drop(tunnel);
        });

        let udp_clients = self.udp_clients;
        let udp_run = self.keep_running.clone();

        let udp_task = tokio::spawn(async move {
            let mut buffer = vec![0u8; 2048];
            let mut had_success = false;

            while udp_run.load(Ordering::SeqCst) {
                let rx = match tokio::time::timeout(
                    Duration::from_secs(1),
                    udp.receive_from(&mut buffer),
                )
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(error)) => {
                        if had_success {
                            tracing::error!(?error, "got error");
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    Err(_) => continue,
                };

                had_success = true;

                match rx {
                    UdpTunnelRx::ReceivedPacket { bytes, flow } => {
                        // tracing::info!(bytes, ?flow, "got packet");
                        udp_clients
                            .forward_packet(&flow, &buffer[..bytes])
                            .await
                            .unwrap();
                    }
                    UdpTunnelRx::ConfirmedConnection => {}
                }
            }
        });

        event_broadcaster.send(Event {
            event_inner: EventInner::PlayitggRunnerEvent(PlayitggRunnerEvent {
                playitgg_runner_event_inner: PlayitggRunnerEventInner::RunnerStarted,
            }),
            snowflake: Snowflake::default(),
            details: "Started".to_string(),
            caused_by: CausedBy::System,
        });

        tunnel_task.await.unwrap();
        udp_task.await.unwrap();
    }
}

pub struct LocalLookup {
    pub data: Mutex<Vec<TunnelEntry>>,
}

impl AddressLookup for LocalLookup {
    type Value = SocketAddr;

    fn lookup(&self, ip: IpAddr, port: u16, proto: PortType) -> Option<AddressValue<SocketAddr>> {
        let values = self.data.lock().unwrap();

        for tunnel in &*values {
            if tunnel.port_type != proto && tunnel.port_type != PortType::Both {
                continue;
            }

            if !tunnel.match_ip.matches(ip) {
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

        None
    }
}

impl LocalLookup {
    pub async fn update(&self, tunnels: Vec<AgentTunnel>) {
        let mut entries: Vec<TunnelEntry> = vec![];

        for tunnel in tunnels {
            entries.push(TunnelEntry {
                pub_address: if tunnel
                    .tunnel_type
                    .as_ref()
                    .map(|v| v.eq("minecraft-java"))
                    .unwrap_or(false)
                {
                    tunnel.custom_domain.unwrap_or(tunnel.assigned_domain)
                } else {
                    format!(
                        "{}:{}",
                        tunnel.custom_domain.unwrap_or(tunnel.assigned_domain),
                        tunnel.port.from
                    )
                },
                match_ip: MatchIp {
                    ip_number: tunnel.ip_num,
                    region_id: if tunnel.region_num == 0 {
                        None
                    } else {
                        Some(tunnel.region_num)
                    },
                },
                port_type: tunnel.proto,
                from_port: tunnel.port.from,
                to_port: tunnel.port.to,
                local_start_address: SocketAddr::new(tunnel.local_ip, tunnel.local_port),
            });
        }

        let mut value = self.data.lock().unwrap();
        *value = entries;
    }
}

pub struct TunnelEntry {
    pub pub_address: String,
    pub match_ip: MatchIp,
    pub port_type: PortType,
    pub from_port: u16,
    pub to_port: u16,
    pub local_start_address: SocketAddr,
}

#[derive(Debug)]
pub struct MatchIp {
    pub ip_number: u64,
    pub region_id: Option<u16>,
}

impl MatchIp {
    pub fn new(ip: Ipv6Addr) -> Self {
        let parts = ip.octets();

        /* 6 bytes /48 BGP Routing */
        /* 2 bytes for region id */
        let region_id = u16::from_be_bytes([parts[6], parts[7]]);

        /* 8 bytes for ip number */
        let ip_number = u64::from_be_bytes([
            parts[8], parts[9], parts[10], parts[11], parts[12], parts[13], parts[14], parts[15],
        ]);

        MatchIp {
            ip_number,
            region_id: if region_id == 0 {
                None
            } else {
                Some(region_id)
            },
        }
    }

    fn region_number_v4(ip: Ipv4Addr) -> u16 {
        let octs = ip.octets();

        /* 147.185.221.0/24 (1) */
        if octs[0] == 147 && octs[1] == 185 && octs[2] == 221 {
            1u16
        }
        /* 209.25.140.0/22 (2 to 5) */
        else if octs[0] == 209 && octs[1] == 25 && octs[2] >= 140 && octs[2] <= 143 {
            2u16 + (octs[2] - 140) as u16
        }
        /* 23.133.216.0/24 (6) */
        else if octs[0] == 23 && octs[1] == 133 && octs[2] == 216 {
            6u16
        }
        /* global IP */
        else {
            0
        }
    }

    pub fn matches(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => {
                let octs = ip.octets();

                if octs[3] as u64 != self.ip_number {
                    return false;
                }

                self.region_id
                    .map(|v| v == Self::region_number_v4(ip))
                    .unwrap_or(true)
            }
            IpAddr::V6(ip) => {
                let other = MatchIp::new(ip);
                self.ip_number == other.ip_number && self.region_id == other.region_id
            }
        }
    }
}

pub async fn pipe<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut from: R,
    mut to: W,
    keep_running: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    buffer.resize(2048, 0u8);

    while keep_running.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;

        let received = match tokio::time::timeout(Duration::from_secs(200), from.read(&mut buffer[..])).await {
            Ok(Ok(received)) => {
                received
            }
            Ok(Err(error)) => {
                tracing::error!(?error, "failed to read data");
                return Err(error);
            }
            Err(_) => break,
        };

        if received == 0 {
            tracing::info!("pipe ended due to EOF");
            break;
        }

        to.write_all(&buffer[..received]).await.map_err(|error| {
            tracing::error!(?error, "failed to write data");
            error
        })?;
    }

    Ok(())
}

