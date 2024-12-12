use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use vpn_shared::creds::Credentials;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
  pub listen_address: Ipv4Addr,
  pub listen_port: u16,

  pub max_clients: usize,
  pub client_timeout_secs: u64,

  pub client_credentials: Vec<Credentials>,
}

impl ServerConfig {
  pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
    let contents = std::fs::read_to_string(path)?;
    let config = serde_yml::from_str(&contents)?;
    Ok(config)
  }

  pub fn client_timeout(&self) -> Duration {
    Duration::from_secs(self.client_timeout_secs)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;

  #[test]
  fn test_parse_full_config() {
    let config_str = r#"
            listen_address: "0.0.0.0"
            listen_port: 8000
            max_clients: 10
            client_timeout_secs: 30
            client_credentials:
              - type: "password"
                username: "user1"
                password: "pass1"
              - type: "password"
                username: "user2"
                password: "pass2"
        "#;

    let config: ServerConfig = serde_yml::from_str(config_str).unwrap();

    assert_eq!(config.listen_port, 8000);
    assert_eq!(config.max_clients, 10);
    assert_eq!(config.client_timeout_secs, 30);
    assert_eq!(config.client_credentials.len(), 2);

    let cred1 = Credentials::from_str("user1:pass1").unwrap();
    let cred2 = Credentials::from_str("user2:pass2").unwrap();

    assert!(config.client_credentials.contains(&cred1));
    assert!(config.client_credentials.contains(&cred2));
  }

  #[test]
  fn test_empty_credentials() {
    let config_str = r#"
            listen_address: "0.0.0.0"
            listen_port: 8000
            max_clients: 10
            client_timeout_secs: 30
            client_credentials: []
        "#;

    let config: ServerConfig = serde_yml::from_str(config_str).unwrap();
    assert!(config.client_credentials.is_empty());
  }
}
