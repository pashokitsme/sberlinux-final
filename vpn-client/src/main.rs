mod client;

use std::str::FromStr;

use clap::*;
use client::Client;
use tracing::error;
use vpn_shared::creds::Credentials;

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
  #[arg()]
  listen_adress: String,
  #[arg()]
  listen_port: u16,
  #[arg()]
  server_address: String,
  #[arg(short, long)]
  server_port: u16,

  /// Auth string in format username:password
  #[arg(short, long)]
  auth: Option<String>,
}

#[tokio::main]
async fn real_main(args: Args) -> anyhow::Result<()> {
  let client_builder = Client::builder(args.server_address.parse()?, args.server_port)
    .with_listen_address(args.listen_adress.parse()?, args.listen_port);

  let client_builder = match args.auth.as_deref().map(Credentials::from_str).transpose()? {
    Some(creds) => client_builder.with_creds(creds),
    None => client_builder,
  };

  let client = client_builder.build().await?;

  Ok(())
}

fn main() {
  setup_tracing();
  let args = Args::parse();
  match real_main(args) {
    Ok(_) => {}
    Err(e) => {
      error!("{}", e);
    }
  }
}

fn setup_tracing() {
  tracing_subscriber::fmt::init();
}
