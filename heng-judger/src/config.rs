use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use ubyte::ByteUnit;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Config {
    #[validate]
    pub judger: Judger,

    #[validate]
    pub redis: Redis,

    #[validate]
    pub data: Data,
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
    pub rpc_timeout: u64, // in milliseconds
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Redis {
    #[validate(length(min = 1))]
    pub url: String,

    #[validate(range(max = 64))]
    pub max_open: u64,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Data {
    #[validate(custom = "validate_absolute_path")]
    pub directory: PathBuf,

    pub download_size_limit: ByteUnit,
}

fn validate_absolute_path(path: &PathBuf) -> Result<(), ValidationError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ValidationError::new("requires absolute path"))
    }
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}
