mod config;
mod server;

use std::net::UdpSocket;
use std::time::Duration;

use clap::*;
use tracing::error;

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
  #[arg(short, long)]
  config: String,
}

#[tokio::main]
async fn real_main(args: Args) -> anyhow::Result<()> {
  let config = config::ServerConfig::from_file(&args.config)?;

  let server = server::Server::builder(config.listen_address, config.listen_port)
    .with_max_clients(config.max_clients)
    .with_client_timeout(Duration::from_secs(config.client_timeout_secs))
    .with_client_credentials(config.client_credentials)
    .build()
    .await?;

  Ok(())
}

fn main() {
  let args = Args::parse();

  match real_main(args) {
    Ok(_) => (),
    Err(e) => error!("{}", e),
  }
}

fn setup_logging() {
  tracing_subscriber::fmt().init();
}
