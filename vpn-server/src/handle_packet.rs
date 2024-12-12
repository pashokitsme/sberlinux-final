use anyhow::Result;
use std::net::SocketAddr;

use tracing::debug;
use tracing::error;
use tracing::info;

use vpn_shared::packet::{ClientPacket, ServerPacket};

use crate::server::ConnectedClient;
use crate::server::Server;

pub trait PacketHandler {
  async fn send_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()>;
}

impl Server {
  pub async fn handle(&self, packet: ClientPacket, src_addr: SocketAddr) -> Result<()> {
    match packet {
      ClientPacket::Auth { credentials } => {
        if !self.client_credentials.contains(&credentials) {
          debug!("Authentication failed for {}", src_addr);
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
      }

      ClientPacket::Data(payload) => {
        if !self.clients.contains_key(&src_addr) {
          self.send_packet(ServerPacket::Error("Not authenticated".into()), src_addr).await?;
          return Ok(());
        }

        if let Some(mut client) = self.clients.get_mut(&src_addr) {
          client.last_seen = std::time::Instant::now();
        }

        info!("Received data from client {}: {:?}", src_addr, payload);

        // TODO: Implement actual data handling
      }

      ClientPacket::Ping => {
        if let Some(mut client) = self.clients.get_mut(&src_addr) {
          client.last_seen = std::time::Instant::now();
          self.send_packet(ServerPacket::Pong, src_addr).await?;
        }
      }

      ClientPacket::Disconnect => {
        if self.clients.remove(&src_addr).is_some() {
          info!("Client {} disconnected", src_addr);
        }
      }

      _ => {
        error!("Unknown packet from client {}: {:?}", src_addr, packet);
      }
    }

    Ok(())
  }
}

impl PacketHandler for Server {
  async fn send_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()> {
    _ = tokio::time::timeout(self.client_timeout, self.socket.send_to(&bincode::serialize(&packet)?, addr))
      .await?;
    Ok(())
  }
}
