use std::hash::Hash;
use std::hash::Hasher;

#[non_exhaustive]
pub enum Credentials {
  Password(Password),
}

impl Credentials {
  pub fn hashed(&self) -> Vec<u8> {
    match self {
      Credentials::Password(password) => password.hashed(),
    }
  }
}

pub struct Password {
  username: String,
  password: String,
}

impl Password {
  pub fn new<S: AsRef<str>>(username: S, password: S) -> Self {
    Self { username: username.as_ref().to_string(), password: password.as_ref().to_string() }
  }

  pub fn hashed(&self) -> Vec<u8> {
    let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
    self.hash(&mut hasher);
    hasher.digest().to_le_bytes().to_vec()
  }
}

impl Hash for Password {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.username.hash(state);
    self.password.hash(state);
  }
}
