use std::net::UdpSocket;

use clap::*;

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
  #[arg()]
  listen_address: String,
  #[arg(short, long)]
  listen_port: u16,
}

fn main() {
  let args = Args::parse();

  // UdpSocket::bind(args.)
}
