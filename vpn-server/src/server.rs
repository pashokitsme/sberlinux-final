use dashmap::DashMap;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::net::UdpSocket;
use vpn_shared::packet::EncryptedPacket;
use vpn_shared::packet::Key;
use vpn_shared::packet::ServerPacket;
use vpn_shared::packet::KEY_SIZE;

use tracing::error;
use tracing::info;

use vpn_shared::creds::Credentials;

use crate::handle_packet::PacketHandler;

pub struct ConnectedClient {
  pub addr: SocketAddr,
  pub last_seen: Instant,
  pub timeout: Duration,
  pub key: Key,
}

impl ConnectedClient {
  pub fn new(key: Key, addr: SocketAddr, timeout: Duration) -> Self {
    Self { addr, last_seen: Instant::now(), timeout, key }
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

    let server = Arc::new(self);

    let cleanup_server = server.clone();
    let cleanup_interval = server.client_timeout / 2;
    tokio::spawn(async move {
      loop {
        cleanup_server.cleanup_inactive_clients().await;
        tokio::time::sleep(cleanup_interval).await;
      }
    });

    let mut buf = vec![0u8; 65536];

    loop {
      let (len, src_addr) = server.socket.recv_from(&mut buf).await?;

      let packet = EncryptedPacket::from_bytes(&buf[..len])?;
      let key = server.clients.get(&src_addr).map(|c| c.key).unwrap_or([0u8; KEY_SIZE]);

      match packet.decrypt(&key) {
        Ok(packet) => {
          let server = server.clone();
          tokio::spawn(async move {
            if let Err(e) = server.handle(packet, src_addr).await {
              error!("Error handling packet from {}: {}", src_addr, e);
            }
          });
        }
        Err(e) => {
          error!("Error decrypting/deserializing packet from {}: {}", src_addr, e);
        }
      }
    }
  }

  pub async fn assert_auth(&self, src_addr: SocketAddr) -> anyhow::Result<()> {
    if !self.clients.contains_key(&src_addr) {
      self.send_packet(ServerPacket::AuthError("Invalid credentials".into()), src_addr).await?;
      anyhow::bail!("Invalid credentials for {}", src_addr);
    }

    let mut client = self.clients.get_mut(&src_addr).unwrap();
    client.last_seen = Instant::now();

    Ok(())
  }

  pub fn get_client_key(&self, src_addr: SocketAddr) -> Key {
    self.clients.get(&src_addr).map(|c| c.key).unwrap_or([0u8; KEY_SIZE])
  }

  async fn cleanup_inactive_clients(&self) {
    let clients_to_remove: Vec<_> =
      self.clients.iter().filter(|client| client.is_expired()).map(|client| client.addr).collect();

    for addr in clients_to_remove {
      info!("Disconnecting stale client {}", addr);
      self.clients.remove(&addr);

      if let Err(e) =
        self.send_packet(ServerPacket::Disconnect { reason: "Stale connection".into() }, addr).await
      {
        error!("Failed to send disconnect packet to {}: {}", addr, e);
      }
    }
  }
}
