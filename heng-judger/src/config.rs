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

    #[validate]
    pub executor: Executor,
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

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Executor {
    #[validate(custom = "validate_absolute_path")]
    pub nsjail_config: PathBuf,

    #[validate(custom = "validate_absolute_path")]
    pub workspace_root: PathBuf,

    pub uid: u32,
    pub gid: u32,

    #[validate]
    pub hard_limit: HardLimit,

    #[validate]
    pub compilers: Compilers,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct HardLimit {
    pub cpu_time: u64,
    pub memory: ByteUnit,
    pub output: ByteUnit,
    pub pids: u32,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Compilers {
    #[validate(custom = "validate_absolute_path")]
    pub c: PathBuf,

    #[validate(custom = "validate_absolute_path")]
    pub cpp: PathBuf,

    #[validate(custom = "validate_absolute_path")]
    pub java: PathBuf,

    #[validate(custom = "validate_absolute_path")]
    pub rust: PathBuf,
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
