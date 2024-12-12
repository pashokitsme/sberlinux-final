use std::hash::Hash;
use std::hash::Hasher;
use std::str::FromStr;

#[non_exhaustive]
pub enum Credentials {
  Password(Password),
}

impl FromStr for Credentials {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let (username, password) = s.split_once(':').ok_or(anyhow::anyhow!("Invalid auth string: missing colon"))?;
    Ok(Self::Password(Password::new(username, password)))
  }
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
