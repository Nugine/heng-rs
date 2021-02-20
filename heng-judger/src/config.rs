use std::fs;
use std::path::Path;

use anyhow::Result;
use once_cell::sync::OnceCell;
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

static GLOBAL_CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn init_from_file(path: impl AsRef<Path>) -> Result<&'static Config> {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        let _ = GLOBAL_CONFIG.set(config);
        Ok(GLOBAL_CONFIG.get().unwrap())
    }

    pub fn global() -> &'static Config {
        GLOBAL_CONFIG.get().unwrap()
    }
}
