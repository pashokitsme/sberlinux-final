use chacha20poly1305::aead::Aead;
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::KeyInit;
use chacha20poly1305::Tag;
use rand::RngCore;

use serde::Deserialize;
use serde::Serialize;

use crate::creds::Credentials;

pub const NONCE_SIZE: usize = 12;
pub const KEY_SIZE: usize = 32;
pub const TAG_SIZE: usize = 16;

#[derive(Debug)]
pub struct EncryptedPacket {
  nonce: [u8; NONCE_SIZE],
  data: Vec<u8>,
  tag: Tag,
}

impl EncryptedPacket {
  pub fn encrypt<P: Serialize>(key: &[u8; KEY_SIZE], packet: &P) -> anyhow::Result<Self> {
    let packet = bincode::serialize(packet)?;
    let cipher = ChaCha20Poly1305::new(key.into());

    let mut nonce = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce);

    let ciphertext = cipher
      .encrypt((&nonce).into(), packet.as_slice())
      .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    let tag_start = ciphertext.len() - TAG_SIZE;
    let tag = Tag::clone_from_slice(&ciphertext[tag_start..]);

    Ok(Self { nonce, data: ciphertext[..tag_start].to_vec(), tag })
  }

  pub fn decrypt<P: for<'de> Deserialize<'de>>(&self, key: &[u8; KEY_SIZE]) -> anyhow::Result<P> {
    let cipher = ChaCha20Poly1305::new(key.into());

    let mut ciphertext = self.data.clone();
    ciphertext.extend_from_slice(&self.tag);

    let decrypted: Vec<u8> = cipher
      .decrypt((&self.nonce).into(), ciphertext.as_ref())
      .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

    bincode::deserialize(&decrypted).map_err(|e| anyhow::anyhow!("Deserialization failed: {}", e))
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(NONCE_SIZE + self.data.len() + TAG_SIZE);
    bytes.extend_from_slice(&self.nonce);
    bytes.extend_from_slice(&self.data);
    bytes.extend_from_slice(&self.tag);
    bytes
  }

  pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
    if bytes.len() < NONCE_SIZE + TAG_SIZE {
      anyhow::bail!("Packet too short");
    }

    let nonce: [u8; NONCE_SIZE] =
      bytes[..NONCE_SIZE].try_into().map_err(|_| anyhow::anyhow!("Invalid nonce"))?;

    let tag_start = bytes.len() - TAG_SIZE;
    let tag = Tag::clone_from_slice(&bytes[tag_start..]);

    let data = bytes[NONCE_SIZE..tag_start].to_vec();

    Ok(Self { nonce, data, tag })
  }
}

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum ClientPacket {
  Auth(Credentials),
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
