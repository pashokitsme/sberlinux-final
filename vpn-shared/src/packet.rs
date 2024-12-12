pub enum Packet {
  Auth { username: String, password_hash: Vec<u8> },
  Data(Vec<u8>),
  Ping,
}
