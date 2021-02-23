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

    #[validate]
    pub judger: Judger,

    #[validate]
    pub auth: Auth,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Server {
    #[validate(length(min = 1))]
    pub address: String,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Redis {
    #[validate(length(min = 1))]
    pub url: String,

    #[validate(range(max = 64))]
    pub max_open: u64,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Judger {
    #[validate(range(max = 60000))]
    pub token_ttl: u64, // ms

    #[validate(range(min = 1000, max = 60000))]
    pub rpc_timeout: u64, // ms
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Auth {
    #[validate(length(min = 1))]
    pub root_access_key: String,

    #[validate(length(min = 1))]
    pub root_secret_key: String,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}
