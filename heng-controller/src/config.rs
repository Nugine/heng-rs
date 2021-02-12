use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Config {
    #[validate]
    pub server: Server,

    #[validate]
    pub redis: Redis,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Server {
    #[validate(length(min = 1))]
    pub host: String,

    pub port: u16,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Redis {
    #[validate(length(min = 1))]
    pub url: String,

    #[validate(length(min = 1))]
    pub key_prefix: String,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = fs::read_to_string(&path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }
}
