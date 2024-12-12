use serde::Deserialize;
use serde::Serialize;

use crate::creds::Credentials;

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum ClientPacket {
  Auth { credentials: Credentials },
  Data(Vec<u8>),
  Ping,
  Disconnect,
}

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum ServerPacket {
  AuthOk,
  AuthError(String),
  Data(Vec<u8>),
  Pong,
  Error(String),
  Disconnect { reason: String },
}
