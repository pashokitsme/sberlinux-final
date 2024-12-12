use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum ClientPacket {
  Auth { username: String, password_hashed: Vec<u8> },
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
  Error(String),
  Pong,
  Disconnect { reason: String },
}
