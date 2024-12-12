use anyhow::Result;
use std::net::SocketAddr;
use tracing::warn;
use vpn_shared::creds::Credentials;
use vpn_shared::packet::fill_random_bytes;
use vpn_shared::packet::EncryptedPacket;
use vpn_shared::packet::Key;
use vpn_shared::packet::KEY_SIZE;

use tracing::error;
use tracing::info;

use vpn_shared::packet::{ClientPacket, ServerPacket};

use crate::server::ConnectedClient;
use crate::server::Server;

#[allow(async_fn_in_trait)]
pub trait PacketHandler {
  async fn send_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()>;
  async fn send_unencrypted_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()>;
  async fn handle_auth(&self, credentials: Credentials, src_addr: SocketAddr) -> Result<()>;
  async fn handle_data(&self, payload: Vec<u8>, src_addr: SocketAddr) -> Result<()>;
  async fn handle_ping(&self, src_addr: SocketAddr) -> Result<()>;
  async fn handle_disconnect(&self, src_addr: SocketAddr) -> Result<()>;
  async fn handle_key_exchange(&self, client_key: Key, src_addr: SocketAddr) -> Result<()>;
}

impl Server {
  pub async fn handle(&self, packet: ClientPacket, src_addr: SocketAddr) -> Result<()> {
    match packet {
      ClientPacket::Auth(credentials) => self.handle_auth(credentials, src_addr).await?,
      ClientPacket::Data(payload) => self.handle_data(payload, src_addr).await?,
      ClientPacket::Ping => self.handle_ping(src_addr).await?,
      ClientPacket::Disconnect => self.handle_disconnect(src_addr).await?,
      ClientPacket::KeyExchange(client_key) => self.handle_key_exchange(client_key, src_addr).await?,
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
      self.clients.remove(&src_addr);
      self.send_packet(ServerPacket::AuthError("Server is full".into()), src_addr).await?;
      return Ok(());
    }

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
    let encrypted_packet = EncryptedPacket::encrypt(&self.get_client_key(addr), &packet)?;
    _ = tokio::time::timeout(self.client_timeout, self.socket.send_to(&encrypted_packet.to_bytes(), addr))
      .await?;
    Ok(())
  }

  async fn send_unencrypted_packet(&self, packet: ServerPacket, addr: SocketAddr) -> Result<()> {
    let encrypted_packet = EncryptedPacket::encrypt(&[0u8; KEY_SIZE], &packet)?;
    _ = tokio::time::timeout(self.client_timeout, self.socket.send_to(&encrypted_packet.to_bytes(), addr))
      .await?;
    Ok(())
  }

  async fn handle_key_exchange(&self, client_key: Key, src_addr: SocketAddr) -> Result<()> {
    let mut server_key = [0u8; KEY_SIZE];
    fill_random_bytes(&mut server_key);

    let mut session_key = [0u8; KEY_SIZE];
    for i in 0..KEY_SIZE {
      session_key[i] = client_key[i] ^ server_key[i];
    }

    let client = ConnectedClient::new(session_key, src_addr, self.client_timeout);

    self.clients.insert(src_addr, client);

    if let Some(mut client) = self.clients.get_mut(&src_addr) {
      client.key = session_key;
      client.last_seen = std::time::Instant::now();
    }

    self.send_unencrypted_packet(ServerPacket::KeyExchange(server_key), src_addr).await?;

    info!("Key exchange completed for client {}", src_addr);
    Ok(())
  }
}
