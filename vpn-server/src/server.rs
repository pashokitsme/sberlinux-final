use dashmap::DashMap;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::net::UdpSocket;

use tracing::error;
use tracing::info;

use crate::handle_packet::PacketHandler;

use vpn_shared::creds::Credentials;
use vpn_shared::packet::ClientPacket;

pub struct ConnectedClient {
  pub addr: SocketAddr,
  pub last_seen: Instant,
  pub timeout: Duration,
}

impl ConnectedClient {
  pub fn new(addr: SocketAddr, timeout: Duration) -> Self {
    Self { addr, last_seen: Instant::now(), timeout }
  }

  pub fn is_expired(&self) -> bool {
    Instant::now().duration_since(self.last_seen) > self.timeout
  }
}

pub struct ServerBuilder {
  listen_address: Ipv4Addr,
  listen_port: u16,
  max_clients: Option<usize>,
  client_timeout: Option<Duration>,
  client_credentials: Option<Vec<Credentials>>,
}

pub struct Server {
  pub socket: UdpSocket,
  pub listen_address: Ipv4Addr,
  pub listen_port: u16,
  pub max_clients: usize,
  pub client_timeout: Duration,
  pub client_credentials: Vec<Credentials>,
  pub clients: Arc<DashMap<SocketAddr, ConnectedClient>>,
}

impl ServerBuilder {
  pub fn new(listen_address: Ipv4Addr, listen_port: u16) -> Self {
    Self { listen_address, listen_port, max_clients: None, client_timeout: None, client_credentials: None }
  }

  pub fn with_max_clients(mut self, max_clients: usize) -> Self {
    self.max_clients = Some(max_clients);
    self
  }

  pub fn with_client_timeout(mut self, timeout: Duration) -> Self {
    self.client_timeout = Some(timeout);
    self
  }

  pub fn with_client_credentials(mut self, credentials: Vec<Credentials>) -> Self {
    self.client_credentials = Some(credentials);
    self
  }

  pub async fn build(self) -> anyhow::Result<Server> {
    let bind_addr = format!("{}:{}", self.listen_address, self.listen_port);
    let server = Server {
      socket: UdpSocket::bind(bind_addr).await?,
      listen_address: self.listen_address,
      listen_port: self.listen_port,
      max_clients: self.max_clients.unwrap_or(10),
      client_timeout: self.client_timeout.unwrap_or(Duration::from_secs(30)),
      client_credentials: self.client_credentials.unwrap_or_default(),
      clients: Arc::new(DashMap::new()),
    };

    Ok(server)
  }
}

impl Server {
  pub fn builder(listen_address: Ipv4Addr, listen_port: u16) -> ServerBuilder {
    ServerBuilder::new(listen_address, listen_port)
  }

  pub async fn run(self) -> anyhow::Result<()> {
    info!("Starting server on {}:{}", self.listen_address, self.listen_port);

    let clients = self.clients.clone();
    let cleanup_interval = self.client_timeout / 2;

    tokio::spawn(async move {
      loop {
        Self::cleanup_inactive_clients(&clients).await;
        tokio::time::sleep(cleanup_interval).await;
      }
    });

    let mut buf = vec![0u8; 65536];

    loop {
      match self.socket.recv_from(&mut buf).await {
        Ok((len, src_addr)) => match bincode::deserialize::<ClientPacket>(&buf[..len]) {
          Ok(packet) => {
            if let Err(e) = self.handle(packet, src_addr).await {
              error!("Error handling packet from {}: {}", src_addr, e);
            }
          }
          Err(e) => {
            error!("Error deserializing packet from {}: {}", src_addr, e);
          }
        },
        Err(e) => {
          error!("Error receiving packet: {}", e);
        }
      }
    }
  }

  async fn cleanup_inactive_clients(clients: &DashMap<SocketAddr, ConnectedClient>) {
    clients.retain(|_, client| {
      let is_active = !client.is_expired();
      if !is_active {
        info!("Removing inactive client {}", client.addr);
      }
      is_active
    });
  }
}
