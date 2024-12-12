use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::UdpSocket;

use vpn_shared::creds::Credentials;

pub struct ServerBuilder {
  listen_address: Ipv4Addr,
  listen_port: u16,
  max_clients: Option<usize>,
  client_timeout: Option<Duration>,
  client_credentials: Option<Vec<Credentials>>,
}

pub struct Server {
  socket: UdpSocket,
  listen_address: Ipv4Addr,
  listen_port: u16,
  max_clients: usize,
  client_timeout: Duration,
  client_credentials: Vec<Credentials>,
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
    };

    Ok(server)
  }
}

impl Server {
  pub fn builder(listen_address: Ipv4Addr, listen_port: u16) -> ServerBuilder {
    ServerBuilder::new(listen_address, listen_port)
  }
}
