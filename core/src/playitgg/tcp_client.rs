use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::Instrument;

use playit_agent_common::agent_config::get_match_ip;
use playit_agent_common::{ClaimInstruction, NewClient};

use playit_agent_core_v09::lan_address::LanAddress;
use playit_agent_core_v09::now_milli;

#[derive(Default)]
pub struct TcpClients {
    client_id: AtomicU64,
    clients: RwLock<TcpClientLookup>,
}

#[derive(Default)]
struct TcpClientLookup {
    active: HashSet<(SocketAddr, SocketAddr)>,
    clients: Vec<Arc<TcpClient>>,
}

#[derive(Debug)]
pub enum SetupFailReason {
    AlreadyProcessing,
    TunnelServerNoConnect(std::io::Error),
    LocalServerNoConnect(std::io::Error),
}

impl TcpClients {
    pub fn next_id(&self) -> u64 {
        self.client_id.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn client_count(&self) -> usize {
        self.clients.read().await.clients.len()
    }

    pub async fn client_for_tunnel(
        &self,
        tunnel_ip: IpAddr,
        tunnel_from_port: u16,
        tunnel_to_port: u16,
    ) -> Vec<Arc<TcpClient>> {
        let mut res = Vec::new();
        let lookup = self.clients.read().await;

        let search = get_match_ip(tunnel_ip);

        for client in lookup.clients.iter() {
            let client_tunnel_addr = get_match_ip(client.tunnel_addr.ip());
            let matches = client_tunnel_addr == search
                && tunnel_from_port <= client.tunnel_addr.port()
                && client.tunnel_addr.port() < tunnel_to_port;

            if matches {
                res.push(client.clone());
            }
        }

        res
    }

    pub async fn add_client(&self, client: Arc<TcpClient>) {
        let mut lookup = self.clients.write().await;
        lookup
            .active
            .insert((client.client_addr, client.tunnel_addr));
        lookup.clients.push(client);
    }

    pub async fn remove_client(&self, client_id: u64) {
        let mut lookup = self.clients.write().await;

        let clients = &mut lookup.clients;
        for i in 0..clients.len() {
            if clients[i].id == client_id {
                let removed = clients.remove(i);
                lookup
                    .active
                    .remove(&(removed.client_addr, removed.tunnel_addr));
                return;
            }
        }
    }
}

pub struct TcpClient {
    pub id: u64,
    pub running: AtomicU64,

    pub from_tunnel_bytes: AtomicUsize,
    pub to_tunnel_bytes: AtomicUsize,
    pub last_msg_at: AtomicU64,

    pub host_local_addr: SocketAddr,
    pub host_peer_addr: SocketAddr,
    pub client_local_addr: SocketAddr,
    pub client_peer_addr: SocketAddr,

    pub tunnel_addr: SocketAddr,
    pub client_addr: SocketAddr,

    pub clients: Arc<TcpClients>,
}

pub struct TcpConnection {
    pub client_token: Vec<u8>,
    pub peer_address: SocketAddr,
    pub claim_addr: SocketAddr,
    pub tunnel_addr: SocketAddr,
    pub span: tracing::Span,
}

const RESP_LEN: usize = 8;

impl TcpConnection {
    pub async fn spawn(
        client: NewClient,
        host_addr: SocketAddr,
        tcp_clients: Arc<TcpClients>,
        keep_running: Arc<AtomicBool>,
    ) -> Result<ActiveTcpConnection, SetupFailReason> {
        let connection_flow = (client.peer_addr, client.connect_addr);
        let tcp_clients_copy = tcp_clients.clone();

        {
            let mut lookup = tcp_clients_copy.clients.write().await;
            if !lookup.active.insert(connection_flow) {
                return Err(SetupFailReason::AlreadyProcessing);
            }
        }

        let ClaimInstruction { address, token } = client.claim_instructions.into_instruction();

        let span = tracing::info_span!("tcp client",
            tunnel_address = %address,
            local_address = %host_addr,
        );

        let conn_span = span.clone();

        let result = async {
            tracing::info!("new client");

            let tcp_conn = TcpConnection {
                client_token: token,
                peer_address: client.peer_addr,
                claim_addr: address,
                tunnel_addr: client.connect_addr,
                span: conn_span,
            };

            let ready = match tcp_conn.establish().await {
                Ok(v) => v,
                Err(error) => {
                    tracing::error!(?error, "failed to establish connection to tunnel server");
                    return Err(SetupFailReason::TunnelServerNoConnect(error));
                }
            };

            let active = match ready
                .connect_to_host(host_addr, tcp_clients, keep_running)
                .await
            {
                Ok(v) => v,
                Err(error) => {
                    tracing::error!(?error, "failed to connect to local service");
                    return Err(SetupFailReason::LocalServerNoConnect(error));
                }
            };

            tracing::info!("connection setup");

            Ok(active)
        }
        .instrument(span)
        .await;

        match result {
            Ok(v) => Ok(v),
            Err(error) => {
                let mut lookup = tcp_clients_copy.clients.write().await;
                lookup.active.remove(&connection_flow);
                Err(error)
            }
        }
    }

    pub async fn establish(self) -> std::io::Result<ReadyTcpConnection> {
        let span = self.span.clone();

        async {
            let mut stream = match TcpStream::connect(self.claim_addr).await {
                Ok(v) => v,
                Err(error) => {
                    tracing::error!(?error, "failed to connect to tunnel server");
                    return Err(error);
                }
            };

            if let Err(error) = stream.set_nodelay(true) {
                tracing::warn!(?error, "failed to set TCP no delay");
            }

            if let Err(error) = stream.write_all(&self.client_token).await {
                tracing::error!(?error, "failed to send tcp claim token");
                return Err(error);
            }

            let mut resp = [0u8; RESP_LEN];
            let size = match stream.read_exact(&mut resp).await {
                Ok(v) => v,
                Err(error) => {
                    tracing::error!(?error, "failed to complete TCP new client handshake");
                    return Err(error);
                }
            };

            if size != RESP_LEN {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "did not get valid response",
                ));
            }

            Ok(ReadyTcpConnection {
                connection: stream,
                peer_addr: self.peer_address,
                tunnel_addr: self.tunnel_addr,
                span,
            })
        }
        .instrument(self.span)
        .await
    }
}

pub struct ReadyTcpConnection {
    connection: TcpStream,
    peer_addr: SocketAddr,
    tunnel_addr: SocketAddr,
    span: tracing::Span,
}

impl ReadyTcpConnection {
    pub async fn connect_to_host(
        self,
        host_addr: SocketAddr,
        clients: Arc<TcpClients>,
        keep_running: Arc<AtomicBool>,
    ) -> std::io::Result<ActiveTcpConnection> {
        async {
            let conn = match LanAddress::tcp_socket(true, self.peer_addr, host_addr).await {
                Ok(v) => v,
                Err(error) => {
                    tracing::error!(
                        ?error,
                        "failed to connect to local server (is your server running?)"
                    );
                    return Err(error);
                }
            };

            if let Err(error) = conn.set_nodelay(true) {
                tracing::warn!(?error, "failed to set no delay");
            }

            let client = Arc::new(TcpClient {
                id: clients.next_id(),
                running: AtomicU64::new(3),
                from_tunnel_bytes: Default::default(),
                to_tunnel_bytes: Default::default(),

                last_msg_at: Default::default(),
                host_local_addr: conn.local_addr().unwrap(),
                host_peer_addr: conn.peer_addr().unwrap(),

                client_local_addr: self.connection.local_addr().unwrap(),
                client_peer_addr: self.connection.peer_addr().unwrap(),

                tunnel_addr: self.tunnel_addr,
                client_addr: self.peer_addr,

                clients: clients.clone(),
            });

            clients.add_client(client.clone()).await;

            let (host_rx, host_tx) = conn.into_split();
            let (tunnel_rx, tunnel_tx) = self.connection.into_split();

            Ok(ActiveTcpConnection {
                client: client.clone(),
                host_to_tunnel: tokio::spawn(
                    pipe(
                        host_rx,
                        tunnel_tx,
                        client.clone(),
                        keep_running.clone(),
                        false,
                    )
                    .instrument(tracing::info_span!("local to tunnel")),
                ),
                tunnel_to_host: tokio::spawn(
                    pipe(tunnel_rx, host_tx, client, keep_running, true)
                        .instrument(tracing::info_span!("tunnel to local")),
                ),
            })
        }
        .instrument(self.span)
        .await
    }
}

#[allow(dead_code)]
pub struct ActiveTcpConnection {
    pub client: Arc<TcpClient>,
    host_to_tunnel: JoinHandle<std::io::Result<()>>,
    tunnel_to_host: JoinHandle<std::io::Result<()>>,
}

impl ActiveTcpConnection {
    pub async fn wait(self) {
        if let Err(error) = self.host_to_tunnel.await {
            tracing::error!(?error, "error joining host=>tunnel");
        }
        if let Err(error) = self.tunnel_to_host.await {
            tracing::error!(?error, "error joining tunnel=>host");
        }
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub running: AtomicUsize,
    pub from_tunnel: AtomicUsize,
    pub to_tunnel: AtomicUsize,
}

async fn pipe(
    mut from: OwnedReadHalf,
    mut to: OwnedWriteHalf,
    client: Arc<TcpClient>,
    keep_running: Arc<AtomicBool>,
    from_tunnel: bool,
) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    buffer.resize(2048, 0u8);

    let r = async {
        loop {
            tokio::task::yield_now().await;

            if !keep_running.load(Ordering::SeqCst) {
                return Ok(());
            }

            let received =
                match tokio::time::timeout(Duration::from_secs(60), from.read(&mut buffer[..]))
                    .await
                {
                    Ok(Ok(received)) => {
                        client.last_msg_at.store(now_milli(), Ordering::SeqCst);
                        received
                    }
                    Ok(Err(error)) => {
                        tracing::error!(?error, "failed to read data");
                        return Err(error);
                    }
                    Err(_) => {
                        let last_msg = client.last_msg_at.load(Ordering::SeqCst);

                        if now_milli() - last_msg > 60_000 {
                            tracing::error!("connection timed out");
                            break;
                        }

                        continue;
                    }
                };

            if received == 0 {
                tracing::info!("pipe ended due to EOF");
                break;
            }

            if from_tunnel {
                &client.from_tunnel_bytes
            } else {
                &client.to_tunnel_bytes
            }
            .fetch_add(received, Ordering::SeqCst);

            to.write_all(&buffer[..received]).await.map_err(|error| {
                tracing::error!(?error, "failed to write data");
                error
            })?;
        }

        Ok(())
    }
    .await;

    let done = if from_tunnel {
        client.running.fetch_sub(2, Ordering::SeqCst) == 2
    } else {
        client.running.fetch_sub(1, Ordering::SeqCst) == 1
    };

    if done {
        client.clients.remove_client(client.id).await;
    }

    r
}
