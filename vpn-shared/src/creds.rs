use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;

impl FromStr for Credentials {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let (username, password) =
      s.split_once(':').ok_or(anyhow::anyhow!("Invalid auth string: missing colon"))?;
    Ok(Self::new(username, password))
  }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct Credentials {
  username: String,
  password: String,
}

impl Credentials {
  pub fn new<S: AsRef<str>>(username: S, password: S) -> Self {
    Self { username: username.as_ref().to_string(), password: password.as_ref().to_string() }
  }

  pub fn username(&self) -> &str {
    &self.username
  }
}
