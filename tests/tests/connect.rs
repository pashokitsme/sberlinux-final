use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;

use tokio::time::sleep;
use vpn_client::client::Client;
use vpn_server::server::Server;
use vpn_shared::creds::Credentials;

fn init_logging() {
  static INIT: Once = Once::new();
  INIT.call_once(|| {
    tracing_subscriber::fmt::init();
  });
}

#[tokio::test]
async fn test_client_server_connection() -> anyhow::Result<()> {
  init_logging();

  let credentials = Credentials::from_str("test_user:test_pass")?;

  let server = Server::builder(Ipv4Addr::LOCALHOST, 8000)
    .with_max_clients(10)
    .with_client_timeout(Duration::from_secs(30))
    .with_client_credentials(vec![credentials.clone()])
    .build()
    .await?;

  let server_handle = tokio::spawn(async move {
    if let Err(e) = server.run().await {
      eprintln!("Server error: {}", e);
    }
  });

  sleep(Duration::from_millis(100)).await;

  let client = Client::builder(Ipv4Addr::LOCALHOST, 8000)
    .with_listen_address(Ipv4Addr::LOCALHOST, 0)
    .with_connect_timeout(Duration::from_secs(5))
    .with_creds(credentials)
    .build()
    .await?;

  let client_handle = tokio::spawn(async move {
    if let Err(e) = client.run().await {
      eprintln!("Client error: {}", e);
    }
  });

  sleep(Duration::from_secs(2)).await;

  client_handle.abort();
  server_handle.abort();

  Ok(())
}

#[tokio::test]
async fn test_client_auth_failure() -> anyhow::Result<()> {
  init_logging();

  let server_creds = Credentials::from_str("test_user:correct_pass")?;
  let client_creds = Credentials::from_str("test_user:wrong_pass")?;

  let server = Server::builder(Ipv4Addr::LOCALHOST, 8001)
    .with_max_clients(10)
    .with_client_timeout(Duration::from_secs(30))
    .with_client_credentials(vec![server_creds])
    .build()
    .await?;

  let server_handle = tokio::spawn(async move {
    if let Err(e) = server.run().await {
      eprintln!("Server error: {}", e);
    }
  });

  sleep(Duration::from_millis(100)).await;

  let client = Client::builder(Ipv4Addr::LOCALHOST, 8001)
    .with_listen_address(Ipv4Addr::LOCALHOST, 0)
    .with_connect_timeout(Duration::from_secs(5))
    .with_creds(client_creds)
    .build()
    .await?;

  match client.run().await {
    Ok(_) => panic!("Expected authentication to fail"),
    Err(e) => assert!(e.to_string().contains("Authentication failed")),
  }

  server_handle.abort();
  Ok(())
}
