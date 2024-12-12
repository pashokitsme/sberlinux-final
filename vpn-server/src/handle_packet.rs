use anyhow::Result;
use std::net::SocketAddr;
use tracing::warn;
use vpn_shared::creds::Credentials;

use tracing::error;
use tracing::info;

use vpn_shared::packet::{ClientPacket, ServerPacket};

use crate::server::ConnectedClient;
use crate::server::Server;

pub trait PacketHandler {
  async fn send_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()>;
  async fn handle_auth(&self, credentials: Credentials, src_addr: SocketAddr) -> Result<()>;
  async fn handle_data(&self, payload: Vec<u8>, src_addr: SocketAddr) -> Result<()>;
  async fn handle_ping(&self, src_addr: SocketAddr) -> Result<()>;
  async fn handle_disconnect(&self, src_addr: SocketAddr) -> Result<()>;
}

impl Server {
  pub async fn handle(&self, packet: ClientPacket, src_addr: SocketAddr) -> Result<()> {
    match packet {
      ClientPacket::Auth(credentials) => self.handle_auth(credentials, src_addr).await?,
      ClientPacket::Data(payload) => self.handle_data(payload, src_addr).await?,
      ClientPacket::Ping => self.handle_ping(src_addr).await?,
      ClientPacket::Disconnect => self.handle_disconnect(src_addr).await?,

      _ => {
        error!("Unknown packet from client {}: {:?}", src_addr, packet);
      }
    }

    Ok(())
  }
}

impl PacketHandler for Server {
  async fn handle_auth(&self, credentials: Credentials, src_addr: SocketAddr) -> Result<()> {
    if !self.client_credentials.contains(&credentials) {
      info!("Authentication failed for {}", src_addr);
      self.send_packet(ServerPacket::AuthError("Invalid credentials".into()), src_addr).await?;
      return Ok(());
    }

    if self.clients.len() >= self.max_clients {
      self.send_packet(ServerPacket::AuthError("Server is full".into()), src_addr).await?;
      return Ok(());
    }

    self.clients.insert(src_addr, ConnectedClient::new(src_addr, self.client_timeout));

    info!("Client {} authenticated successfully", src_addr);
    self.send_packet(ServerPacket::AuthOk, src_addr).await?;

    Ok(())
  }

  async fn handle_data(&self, payload: Vec<u8>, src_addr: SocketAddr) -> Result<()> {
    self.assert_auth(src_addr).await?;

    if let Some(mut client) = self.clients.get_mut(&src_addr) {
      client.last_seen = std::time::Instant::now();
    }

    info!("Received data from client {}: {:?}", src_addr, payload);

    // TODO: Implement actual data handling
    Ok(())
  }

  async fn handle_ping(&self, src_addr: SocketAddr) -> Result<()> {
    self.assert_auth(src_addr).await?;
    info!("Received ping from client {}; sending pong", src_addr);
    self.send_packet(ServerPacket::Pong, src_addr).await?;
    Ok(())
  }

  async fn handle_disconnect(&self, src_addr: SocketAddr) -> Result<()> {
    if self.clients.remove(&src_addr).is_some() {
      info!("Client {} disconnected", src_addr);
    } else {
      warn!("Client {} wasn't connected; ignoring disconnect", src_addr);
    }

    Ok(())
  }

  async fn send_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()> {
    _ = tokio::time::timeout(self.client_timeout, self.socket.send_to(&bincode::serialize(&packet)?, addr))
      .await?;
    Ok(())
  }
}
