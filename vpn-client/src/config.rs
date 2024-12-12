use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use vpn_shared::creds::Credentials;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TunConfig {
  pub name: String,
  pub address: Ipv4Addr,
  pub netmask: Ipv4Addr,
  pub mtu: Option<u16>,

  #[serde(default = "default_tun_up")]
  pub up: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ClientConfig {
  pub server_address: Ipv4Addr,
  pub server_port: u16,

  pub listen_address: Ipv4Addr,
  pub listen_port: u16,

  pub reconnect_interval_secs: u64,
  pub connect_timeout_secs: u64,

  pub credentials: Credentials,

  #[serde(default = "default_tun_config")]
  pub tun: TunConfig,
}

fn default_tun_config() -> TunConfig {
  TunConfig {
    name: "tun0".to_string(),
    address: Ipv4Addr::new(10, 0, 0, 1),
    netmask: Ipv4Addr::new(255, 255, 255, 0),
    mtu: Some(1500),
    up: true,
  }
}

fn default_tun_up() -> bool {
  true
}

impl TunConfig {
  pub fn to_tun_config(&self) -> tun::Configuration {
    let mut config = tun::Configuration::default();

    config.tun_name(&self.name).address(self.address).netmask(self.netmask);

    if self.up {
      config.up();
    }

    if let Some(mtu) = self.mtu {
      config.mtu(mtu);
    }

    config
  }
}

impl ClientConfig {
  pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
    if !path.as_ref().exists() {
      anyhow::bail!("Configuration file not found: {}", path.as_ref().display());
    }

    let contents = std::fs::read_to_string(path)?;
    let config = serde_yml::from_str(&contents)?;
    Ok(config)
  }

  pub fn reconnect_interval(&self) -> Duration {
    Duration::from_secs(self.reconnect_interval_secs)
  }

  pub fn connect_timeout(&self) -> Duration {
    Duration::from_secs(self.connect_timeout_secs)
  }

  pub fn tun_config(&self) -> tun::Configuration {
    self.tun.to_tun_config()
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;

  #[test]
  fn test_parse_config() {
    let config_str = r#"
            server-address: "127.0.0.1"
            server-port: 8000
            listen-address: "0.0.0.0"
            listen-port: 6969
            reconnect-interval-secs: 5
            connect-timeout-secs: 10
            credentials:
              type: "password"
              username: "test_user"
              password: "test_password"
            tun:
              name: "tun0"
              address: "10.0.0.1"
              netmask: "255.255.255.0"
              mtu: 1500
              up: true
        "#;

    let config: ClientConfig = serde_yml::from_str(config_str).unwrap();

    assert_eq!(config.server_port, 8000);
    assert_eq!(config.listen_port, 6969);
    assert_eq!(config.reconnect_interval_secs, 5);
    let Credentials::Password(creds) = config.credentials else {
      panic!("Invalid credentials type");
    };

    assert_eq!(creds.hashed(), Credentials::from_str("test_user:test_password").unwrap().hashed());
  }

  #[test]
  fn test_default_tun_config() {
    let config_str = r#"
            server-address: "127.0.0.1"
            server-port: 8000
            listen-address: "0.0.0.0"
            listen-port: 6969
            reconnect-interval-secs: 5
            connect-timeout-secs: 10
            credentials:
              type: "password"
              username: "test_user"
              password: "test_password"
        "#;

    let config: ClientConfig = serde_yml::from_str(config_str).unwrap();

    assert_eq!(config.tun.name, "tun0");
    assert_eq!(config.tun.address, Ipv4Addr::new(10, 0, 0, 1));
    assert_eq!(config.tun.netmask, Ipv4Addr::new(255, 255, 255, 0));
    assert_eq!(config.tun.mtu, Some(1500));
    assert!(config.tun.up);
  }

  #[test]
  fn test_partial_tun_config() {
    let config_str = r#"
            server-address: "127.0.0.1"
            server-port: 8000
            listen-address: "0.0.0.0"
            listen-port: 6969
            reconnect-interval-secs: 5
            connect-timeout-secs: 10
            credentials:
              type: "password"
              username: "test_user"
              password: "test_password"
            tun:
              name: "vpn0"
              address: "192.168.1.1"
              netmask: "255.255.255.0"
        "#;

    let config: ClientConfig = serde_yml::from_str(config_str).unwrap();

    assert_eq!(config.tun.name, "vpn0");
    assert_eq!(config.tun.address, Ipv4Addr::new(192, 168, 1, 1));
    assert_eq!(config.tun.netmask, Ipv4Addr::new(255, 255, 255, 0));

    assert_eq!(config.tun.mtu, None);
    assert!(config.tun.up);
  }
}
