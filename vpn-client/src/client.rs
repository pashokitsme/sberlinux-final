use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::UdpSocket;

use crate::creds::Credentials;
// use ring::digest;
// use serde::{Serialize, Deserialize};

// #[derive(Serialize, Deserialize)]
enum Packet {
  Auth { username: String, password_hash: Vec<u8> },
  Data(Vec<u8>),
  Ping,
}

pub struct Password {
  username: String,
  password: String,
}

pub struct ClientBuilder {
  server_address: Ipv4Addr,
  server_port: u16,
  listen_address: Ipv4Addr,
  listen_port: u16,
  reconnect_interval: Option<Duration>,
  credentials: Option<Credentials>,
}

pub struct Client {
  socket: UdpSocket,
  server_address: Ipv4Addr,
  server_port: u16,
  listen_address: Ipv4Addr,
  listen_port: u16,
  reconnect_interval: Option<Duration>,
  credentials: Option<Credentials>,
}

impl ClientBuilder {
  pub fn new(server_address: Ipv4Addr, server_port: u16) -> Self {
    Self {
      server_address,
      server_port,
      listen_address: Ipv4Addr::new(0, 0, 0, 0),
      listen_port: 6969,
      reconnect_interval: None,
      credentials: None,
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

  pub async fn build(self) -> anyhow::Result<Client> {
    let client = Client {
      socket: UdpSocket::bind("0.0.0.0:0").await?,
      server_address: self.server_address,
      server_port: self.server_port,
      listen_address: self.listen_address,
      listen_port: self.listen_port,
      reconnect_interval: self.reconnect_interval,
      credentials: self.credentials,
    };

    Ok(client)
  }
}

impl Client {
  pub fn builder(server_address: Ipv4Addr, server_port: u16) -> ClientBuilder {
    ClientBuilder::new(server_address, server_port)
  }
}
