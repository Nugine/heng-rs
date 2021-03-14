use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::error;
use ubyte::ByteUnit;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Config {
    #[validate]
    pub judger: Judger,

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
pub struct Data {
    #[validate(custom = "validate_absolute_path")]
    pub directory: PathBuf,

    pub download_size_limit: ByteUnit,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Executor {
    #[validate(custom = "validate_absolute_path")]
    pub workspace_root: PathBuf,

    pub uid: u32,
    pub gid: u32,

    #[validate]
    pub hard_limit: HardLimit,

    #[validate]
    pub c_cpp: CCpp,

    #[validate]
    pub java: Java,

    #[validate]
    pub javascript: JavaScript,

    #[validate]
    pub python: Python,

    #[validate]
    pub rust: Rust,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct HardLimit {
    pub real_time: u64, // milliseconds
    pub cpu_time: u64,  // milliseconds
    pub memory: ByteUnit,
    pub output: ByteUnit,
    pub pids: u32,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct CCpp {
    pub gcc: PathBuf,
    pub gxx: PathBuf,
    pub mount: Vec<PathBuf>,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Java {
    pub javac: PathBuf,
    pub java: PathBuf,
    pub mount: Vec<PathBuf>,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct JavaScript {
    pub node: PathBuf,
    pub mount: Vec<PathBuf>,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Python {
    pub python: PathBuf,
    pub mount: Vec<PathBuf>,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
pub struct Rust {
    pub rustc: PathBuf,
    pub mount: Vec<PathBuf>,
}

// #[derive(Debug, Clone, Validate, Serialize, Deserialize)]
// pub struct Compilers {
//     #[validate(custom = "validate_binary_file_path")]
//     pub gcc: PathBuf,

//     #[validate(custom = "validate_binary_file_path")]
//     pub gxx: PathBuf,

//     #[validate(custom = "validate_binary_file_path")]
//     pub javac: PathBuf,

//     #[validate(custom = "validate_binary_file_path")]
//     pub rustc: PathBuf,
// }

// #[derive(Debug, Clone, Validate, Serialize, Deserialize)]
// pub struct Runtimes {
//     #[validate(custom = "validate_binary_file_path")]
//     pub java: PathBuf,

//     #[validate(custom = "validate_binary_file_path")]
//     pub node: PathBuf,

//     #[validate(custom = "validate_binary_file_path")]
//     pub python: PathBuf,
// }

fn validate_absolute_path(path: &PathBuf) -> Result<(), ValidationError> {
    if !path.is_absolute() {
        return Err(ValidationError::new("requires absolute path"));
    }
    Ok(())
}

fn validate_binary_file_path(path: &PathBuf) -> Result<(), ValidationError> {
    if !path.is_absolute() {
        return Err(ValidationError::new("requires absolute path"));
    }
    let meta = fs::metadata(path).map_err(|err| {
        error!(%err,"can not get file metadata");
        ValidationError::new("can not get file metadata")
    })?;
    if !meta.is_file() {
        return Err(ValidationError::new("requires regular file"));
    }
    Ok(())
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Config> {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}
