mod client;
mod creds;

use clap::*;

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
  #[arg()]
  listen_adress: String,
  #[arg()]
  listen_port: u16,
  #[arg()]
  address: String,
  #[arg(short, long)]
  port: u16,
}

fn main() {
  let args = Args::parse();
  println!("{:?}", args);
}
