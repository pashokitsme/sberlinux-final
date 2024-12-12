use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::time::sleep;

use tokio::time::Instant;

use tun::AsyncDevice;

use tracing::error;
use tracing::info;

use vpn_shared::creds::Credentials;
use vpn_shared::packet::fill_random_bytes;
use vpn_shared::packet::EncryptedPacket;
use vpn_shared::packet::Key;
use vpn_shared::packet::KEY_SIZE;
use vpn_shared::packet::{ClientPacket, ServerPacket};

pub struct ClientBuilder {
  server_address: Ipv4Addr,
  server_port: u16,
  listen_address: Ipv4Addr,
  listen_port: u16,
  connect_timeout: Option<Duration>,
  credentials: Option<Credentials>,
  tun_config: Option<tun::Configuration>,
}

pub struct Client {
  socket: Arc<UdpSocket>,
  server_address: Ipv4Addr,
  server_port: u16,
  connect_timeout: Duration,
  credentials: Option<Credentials>,
  tun: AsyncDevice,

  last_ping_sent: Instant,
}

impl ClientBuilder {
  pub fn new(server_address: Ipv4Addr, server_port: u16) -> Self {
    Self {
      server_address,
      server_port,
      listen_address: Ipv4Addr::new(0, 0, 0, 0),
      listen_port: 6969,
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
      connect_timeout: self.connect_timeout.unwrap_or(Duration::from_secs(10)),
      credentials: self.credentials,
      tun,
      last_ping_sent: Instant::now(),
    })
  }
}

impl Client {
  pub fn builder(server_address: Ipv4Addr, server_port: u16) -> ClientBuilder {
    ClientBuilder::new(server_address, server_port)
  }

  pub async fn run(mut self) -> anyhow::Result<()> {
    info!("Starting client");

    let key = match self.connect().await {
      Ok(key) => key,
      Err(e) => {
        error!("Failed to connect to server: {}", e);
        return Err(e);
      }
    };

    let (network_tx, mut network_rx) = mpsc::channel(100);

    let server_addr = SocketAddr::new(self.server_address.into(), self.server_port);
    let socket = Arc::clone(&self.socket);

    tokio::spawn(async move {
      let mut buf = vec![0u8; 65536];
      loop {
        match socket.recv_from(&mut buf).await {
          Ok((len, _)) => {
            if let Ok(packet) = EncryptedPacket::from_bytes(&buf[..len]).and_then(|p| p.decrypt(&key)) {
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

    let mut ping_sent_rx = self.start_ping(key, server_addr);

    loop {
      tokio::select! {
        _ = self.serve_tun(key, server_addr) => {}
        Some(packet) = network_rx.recv() => {
          match packet {
            ServerPacket::Data(data) => {
              if let Err(e) = self.tun.write(&data).await {
                error!("Failed to write to tun: {}", e);
              }
            }
            ServerPacket::Error(msg) => {
              error!("Server error: {}", msg);
            }
            ServerPacket::Pong => {
              info!("Ping latency: {:?}", Instant::now().duration_since(self.last_ping_sent));
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
        Some(_) = ping_sent_rx.recv() => {
          self.last_ping_sent = Instant::now();
        }
      }
    }
  }

  async fn connect(&mut self) -> anyhow::Result<Key> {
    let Some(ref credentials) = self.credentials else {
      anyhow::bail!("No credentials provided");
    };

    let server_addr = SocketAddr::new(self.server_address.into(), self.server_port);

    let mut session_key = [0u8; KEY_SIZE];
    fill_random_bytes(&mut session_key);

    let keyexchange_packet =
      EncryptedPacket::encrypt(&[0u8; KEY_SIZE], &ClientPacket::KeyExchange(session_key))?;

    self.socket.send_to(&keyexchange_packet.to_bytes(), server_addr).await?;

    info!("Waiting for key exchange...");
    let mut buf = vec![0u8; 65536];

    match tokio::time::timeout(self.connect_timeout, self.socket.recv_from(&mut buf)).await {
      Ok(Ok((len, _))) => match EncryptedPacket::from_bytes(&buf[..len])?.decrypt(&[0u8; KEY_SIZE])? {
        ServerPacket::KeyExchange(server_key) => {
          for i in 0..KEY_SIZE {
            session_key[i] ^= server_key[i];
          }

          info!("Successfully established secure connection; Authenticating...");
        }
        _ => {
          anyhow::bail!("Failed to establish secure connection");
        }
      },
      _ => {
        anyhow::bail!("Connection handshake timeout");
      }
    }

    let packet = EncryptedPacket::encrypt(&session_key, &ClientPacket::Auth(credentials.clone()))?;
    self.socket.send_to(&packet.to_bytes(), server_addr).await?;

    let mut buf = vec![0u8; 65536];

    match tokio::time::timeout(self.connect_timeout, self.socket.recv_from(&mut buf)).await {
      Ok(Ok((len, _))) => match EncryptedPacket::from_bytes(&buf[..len])?.decrypt(&session_key)? {
        ServerPacket::AuthOk => {
          info!("Authentication successful");
          Ok(session_key)
        }
        ServerPacket::AuthError(message) => anyhow::bail!("Authentication failed: {}", message),
        _ => anyhow::bail!("Unexpected response from server"),
      },
      _ => anyhow::bail!("Connection timeout"),
    }
  }

  async fn serve_tun(&mut self, key: Key, server_addr: SocketAddr) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 65536];
    match self.tun.read(&mut buf).await {
      Ok(len) => {
        let packet = EncryptedPacket::encrypt(&key, &ClientPacket::Data(buf[..len].to_vec()))?;
        match self.socket.send_to(&packet.to_bytes(), server_addr).await {
          Ok(_) => info!("Sent tun packet to server; len: {}", len),
          Err(e) => {
            error!("Failed to send data to server: {}", e);
          }
        }
      }
      Err(e) => {
        anyhow::bail!("Error reading from tun: {}", e);
      }
    }

    Ok(())
  }

  fn start_ping(&self, key: Key, server_addr: SocketAddr) -> Receiver<()> {
    let socket = Arc::clone(&self.socket);
    let interval = Duration::from_secs(5);

    let (tx, rx) = mpsc::channel(1);

    tokio::spawn(async move {
      loop {
        match EncryptedPacket::encrypt(&key, &ClientPacket::Ping) {
          Ok(packet) => {
            if let Err(err) = socket.send_to(&packet.to_bytes(), server_addr).await {
              error!("Failed to send ping: {}", err);
            }
            tx.send(()).await.unwrap();
          }
          Err(e) => {
            error!("Failed to encrypt ping packet: {}", e);
          }
        }

        sleep(interval).await;
      }
    });

    rx
  }
}
