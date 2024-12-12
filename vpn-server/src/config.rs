use std::net::Ipv4Addr;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use vpn_shared::creds::Credentials;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
  #[serde(default = "default_listen_address")]
  pub listen_address: Ipv4Addr,

  #[serde(default = "default_listen_port")]
  pub listen_port: u16,

  #[serde(default = "default_max_clients")]
  pub max_clients: usize,

  #[serde(default = "default_client_timeout_secs")]
  pub client_timeout_secs: u64,

  #[serde(default)]
  pub client_credentials: Vec<Credentials>,

  #[serde(default)]
  pub tun_interface: TunConfig,

  #[serde(default)]
  pub log: LogConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TunConfig {
  #[serde(default = "default_tun_name")]
  pub name: String,

  #[serde(default = "default_tun_address")]
  pub address: Ipv4Addr,

  #[serde(default = "default_tun_netmask")]
  pub netmask: Ipv4Addr,

  #[serde(default = "default_mtu")]
  pub mtu: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogConfig {
  #[serde(default = "default_log_level")]
  pub level: String,

  #[serde(default)]
  pub file: Option<PathBuf>,
}

impl ServerConfig {
  pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
    if !path.as_ref().exists() {
      anyhow::bail!("Configuration file does not exist at {}", path.as_ref().display());
    }

    let content = std::fs::read_to_string(path)?;
    let config = serde_yml::from_str(&content)?;
    Ok(config)
  }
}

impl Default for ServerConfig {
  fn default() -> Self {
    Self {
      listen_address: default_listen_address(),
      listen_port: default_listen_port(),
      max_clients: default_max_clients(),
      client_timeout_secs: default_client_timeout_secs(),
      client_credentials: Vec::new(),
      tun_interface: TunConfig::default(),
      log: LogConfig::default(),
    }
  }
}

impl Default for TunConfig {
  fn default() -> Self {
    Self {
      name: default_tun_name(),
      address: default_tun_address(),
      netmask: default_tun_netmask(),
      mtu: default_mtu(),
    }
  }
}

impl Default for LogConfig {
  fn default() -> Self {
    Self { level: default_log_level(), file: None }
  }
}

fn default_listen_address() -> Ipv4Addr {
  Ipv4Addr::new(0, 0, 0, 0)
}

fn default_listen_port() -> u16 {
  51820
}

fn default_max_clients() -> usize {
  10
}

fn default_client_timeout_secs() -> u64 {
  30
}

fn default_tun_name() -> String {
  "tun0".to_string()
}

fn default_tun_address() -> Ipv4Addr {
  Ipv4Addr::new(10, 0, 0, 1)
}

fn default_tun_netmask() -> Ipv4Addr {
  Ipv4Addr::new(255, 255, 255, 0)
}

fn default_mtu() -> u16 {
  1500
}

fn default_log_level() -> String {
  "info".to_string()
}
