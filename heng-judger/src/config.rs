use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Config {
    #[validate]
    pub judger: Judger,

    #[validate]
    pub redis: Redis,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Judger {
    #[validate(length(min = 1))]
    pub remote_domain: String,

    #[validate(length(min = 1))]
    pub access_key: String,

    #[validate(length(min = 1))]
    pub secret_key: String,

    #[validate(range(min = 1000, max = 60000))]
    pub rpc_timeout: u64,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Redis {
    #[validate(length(min = 1))]
    pub url: String,

    #[validate(range(max = 64))]
    pub max_open: u64,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}
