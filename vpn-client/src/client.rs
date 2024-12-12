use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::sleep;

use tun::AsyncDevice;

use tracing::debug;
use tracing::error;
use tracing::info;

use vpn_shared::creds::Credentials;
use vpn_shared::packet::{ClientPacket, ServerPacket};

pub struct ClientBuilder {
  server_address: Ipv4Addr,
  server_port: u16,
  listen_address: Ipv4Addr,
  listen_port: u16,
  reconnect_interval: Option<Duration>,
  connect_timeout: Option<Duration>,
  credentials: Option<Credentials>,
  tun_config: Option<tun::Configuration>,
}

pub struct Client {
  socket: Arc<UdpSocket>,
  server_address: Ipv4Addr,
  server_port: u16,
  reconnect_interval: Duration,
  connect_timeout: Duration,
  credentials: Option<Credentials>,
  tun: AsyncDevice,
}

impl ClientBuilder {
  pub fn new(server_address: Ipv4Addr, server_port: u16) -> Self {
    Self {
      server_address,
      server_port,
      listen_address: Ipv4Addr::new(0, 0, 0, 0),
      listen_port: 6969,
      reconnect_interval: None,
      connect_timeout: None,
      credentials: None,
      tun_config: None,
    }
  }

  pub fn with_listen_address(mut self, listen_address: Ipv4Addr, listen_port: u16) -> Self {
    self.listen_address = listen_address;
    self.listen_port = listen_port;
    self
  }

  pub fn with_creds(mut self, credentials: Credentials) -> Self {
    self.credentials = Some(credentials);
    self
  }

  pub fn with_reconnect_interval(mut self, reconnect_interval: Duration) -> Self {
    self.reconnect_interval = Some(reconnect_interval);
    self
  }

  pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
    self.connect_timeout = Some(connect_timeout);
    self
  }

  pub fn with_tun_config(mut self, tun_config: tun::Configuration) -> Self {
    self.tun_config = Some(tun_config);
    self
  }

  pub async fn build(self) -> anyhow::Result<Client> {
    let socket = Arc::new(UdpSocket::bind(format!("{}:{}", self.listen_address, self.listen_port)).await?);
    let tun = tun::create_as_async(&self.tun_config.unwrap_or_default())?;

    Ok(Client {
      socket,
      server_address: self.server_address,
      server_port: self.server_port,
      reconnect_interval: self.reconnect_interval.unwrap_or(Duration::from_secs(5)),
      connect_timeout: self.connect_timeout.unwrap_or(Duration::from_secs(10)),
      credentials: self.credentials,
      tun,
    })
  }
}

impl Client {
  pub fn builder(server_address: Ipv4Addr, server_port: u16) -> ClientBuilder {
    ClientBuilder::new(server_address, server_port)
  }

  pub async fn run(mut self) -> anyhow::Result<()> {
    info!("Starting VPN client...");

    if let Err(e) = self.connect().await {
      error!("Failed to connect to server: {}", e);
      return Ok(());
    }

    let (network_tx, mut network_rx) = mpsc::channel(100);

    let server_addr = SocketAddr::new(self.server_address.into(), self.server_port);
    let socket = Arc::clone(&self.socket);

    tokio::spawn(async move {
      let mut buf = vec![0u8; 65536];
      loop {
        match socket.recv_from(&mut buf).await {
          Ok((len, _)) => {
            if let Ok(packet) = bincode::deserialize::<ServerPacket>(&buf[..len]) {
              if network_tx.send(packet).await.is_err() {
                break;
              }
            }
          }
          Err(e) => {
            error!("Error receiving from server: {}", e);
            break;
          }
        }
      }
    });

    let mut tun_buf = vec![0u8; 65536];
    loop {
      tokio::select! {
        result = self.tun.read(&mut tun_buf) => {
          match result {
            Ok(len) => {
              let packet = ClientPacket::Data(tun_buf[..len].to_vec());
              if let Ok(data) = bincode::serialize(&packet) {
                if let Err(e) = self.socket.send_to(&data, server_addr).await {
                  error!("Failed to send data to server: {}", e);
                }
              }
            }
            Err(e) => {
              error!("Error reading from TUN: {}", e);
              break;
            }
          }
        }
        Some(packet) = network_rx.recv() => {
          match packet {
            ServerPacket::Data(data) => {
              if let Err(e) = self.tun.write(&data).await {
                error!("Failed to write to TUN: {}", e);
              }
            }
            ServerPacket::AuthOk => {
              info!("Successfully authenticated with server");
              self.start_ping(server_addr);
            }
            ServerPacket::AuthError(msg) => {
              error!("Authentication failed: {}", msg);
              return Ok(());
            }
            ServerPacket::Error(msg) => {
              error!("Server error: {}", msg);
            }
            ServerPacket::Pong => {
              debug!("Received pong from server");
            }
            ServerPacket::Disconnect { reason } => {
              info!("Disconnected from server: {}", reason);
              return Ok(());
            }
            _ => {
              error!("Unexpected packet from server: {:?}", packet);
            }
          };
        }
      }
    }

    Ok(())
  }

  async fn connect(&self) -> anyhow::Result<()> {
    if let Some(ref credentials) = self.credentials {
      let auth_packet = ClientPacket::Auth {
        username: credentials.username().to_string(),
        password_hashed: credentials.hashed(),
      };
      let server_addr = SocketAddr::new(self.server_address.into(), self.server_port);

      let data = bincode::serialize(&auth_packet)?;
      self.socket.send_to(&data, server_addr).await?;

      let mut buf = vec![0u8; 1024];
      match tokio::time::timeout(self.connect_timeout, self.socket.recv_from(&mut buf)).await {
        Ok(Ok((len, _))) => match bincode::deserialize::<ServerPacket>(&buf[..len]) {
          Ok(packet) => match packet {
            ServerPacket::AuthOk => {
              info!("Successfully connected to server");
              Ok(())
            }
            ServerPacket::AuthError(message) => {
              error!("Authentication failed: {}", message);
              Err(anyhow::anyhow!("Authentication failed: {}", message))
            }
            _ => {
              error!("Unexpected response from server");
              Err(anyhow::anyhow!("Unexpected response from server"))
            }
          },
          Err(e) => {
            error!("Failed to deserialize server response: {}", e);
            Err(anyhow::anyhow!("Failed to deserialize server response: {}", e))
          }
        },
        Ok(Err(e)) => {
          error!("Connection error: {}", e);
          Err(anyhow::anyhow!("Connection error: {}", e))
        }
        Err(_) => {
          error!("Connection timeout");
          Err(anyhow::anyhow!("Connection timeout"))
        }
      }
    } else {
      Err(anyhow::anyhow!("No credentials provided"))
    }
  }

  fn start_ping(&self, server_addr: SocketAddr) {
    let socket = Arc::clone(&self.socket);
    let interval = self.reconnect_interval;

    tokio::spawn(async move {
      loop {
        let ping = ClientPacket::Ping;
        if let Ok(data) = bincode::serialize(&ping) {
          if let Err(e) = socket.send_to(&data, server_addr).await {
            error!("Failed to send ping: {}", e);
            break;
          }
        }
        sleep(interval).await;
      }
    });
  }
}
