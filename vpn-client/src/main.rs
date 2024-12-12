use clap::Parser;
use tracing::error;
use vpn_client::{Client, ClientConfig};

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
  /// Path to the configuration file
  #[arg(short, long)]
  config: String,
}

#[tokio::main]
async fn real_main(args: Args) -> anyhow::Result<()> {
  let config = ClientConfig::from_file(&args.config)?;

  let client = Client::builder(config.server_address, config.server_port)
    .with_listen_address(config.listen_address, config.listen_port)
    .with_connect_timeout(config.connect_timeout())
    .with_tun_config(config.tun_config())
    .with_creds(config.credentials)
    .build()
    .await?;

  client.run().await?;

  Ok(())
}

fn main() {
  let args = Args::parse();
  setup_logging();

  if let Err(e) = real_main(args) {
    error!("{}", e);
  }
}

fn setup_logging() {
  tracing_subscriber::fmt().init();
}
