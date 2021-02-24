use crate::redis::RedisModule;
use crate::Config;

use heng_protocol::common as hp_common;
use heng_utils::crypto::{hex_sha256, to_hex_string};

use std::fs;
use std::io::{BufReader, Write};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use futures::StreamExt;
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::task;
use zip::ZipArchive;

pub struct DataModule {
    redis_module: Arc<RedisModule>,
    directory: PathBuf,
    download_size_limit: u64,
}

pub struct ZipData {
    name: Box<str>,
    hashsum: Box<str>,
    file_path: PathBuf,
    dir_path: PathBuf,
}

impl DataModule {
    pub fn new(config: &Config, redis_module: Arc<RedisModule>) -> Result<Self> {
        let directory = &config.data.directory;
        if !directory.exists() {
            fs::create_dir_all(directory).with_context(|| {
                format!("failed to create directory: path = {}", directory.display())
            })?;
        }

        let download_size_limit = config.data.download_size_limit.as_u64();

        Ok(Self {
            redis_module,
            directory: directory.clone(),
            download_size_limit,
        })
    }

    fn generate_name() -> String {
        let timestamp = Utc::now().timestamp_nanos();
        let rng = rand::thread_rng().gen_range(0..1000);
        format!("{}-{:03}", timestamp, rng)
    }

    async fn download_file(
        &self,
        file: hp_common::File,
        direct_base64: bool,
        target_path: &Path,
    ) -> Result<()> {
        match file {
            hp_common::File::Url { url, hashsum } => {
                let content_hash =
                    download_file(&url, &target_path, self.download_size_limit).await?;
                if let Some(hashsum) = hashsum {
                    if content_hash.as_ref() != hashsum.as_str() {
                        let _ = fs::remove_file(&target_path);
                        anyhow::bail!("file hashsum mismatch")
                    }
                }
            }
            hp_common::File::Direct { content, hashsum } => {
                let base64_decoded;
                let content_bytes = if direct_base64 {
                    base64_decoded = base64::decode(content)?;
                    &*base64_decoded
                } else {
                    content.as_bytes()
                };
                let content_hash: Box<str> = hex_sha256(content_bytes).into();
                if let Some(hashsum) = hashsum {
                    if content_hash.as_ref() != hashsum.as_str() {
                        anyhow::bail!("file hashsum mismatch")
                    }
                }
                fs::write(&target_path, content_bytes)?;
            }
        }

        Ok(())
    }

    async fn unzip(&self, zip: &ZipData) -> Result<()> {
        let file_path = zip.file_path.clone();
        let dir_path = zip.dir_path.clone();
        task::spawn_blocking(move || unzip(&file_path, &dir_path)).await?
    }
}

async fn download_file(url: &str, file_path: &Path, size_limit: u64) -> Result<Box<str>> {
    let res = reqwest::get(url).await?;
    if !res.status().is_success() {
        anyhow::bail!("request failed: status = {}", res.status());
    }

    let mut stream = res.bytes_stream();
    let mut file = fs::File::create(&file_path)?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let len = chunk.len() as u64;
        size += len;
        if size > size_limit {
            anyhow::bail!(
                "body is too large: size = {}, size_limit = {}",
                size,
                size_limit
            );
        }
        hasher.update(&chunk);
        file.write_all(&chunk)?;
    }

    Ok(to_hex_string(hasher.finalize().as_ref()).into())
}

fn unzip(file_path: &Path, target_dir: &Path) -> Result<()> {
    let file = fs::File::open(file_path)?;
    let reader = BufReader::with_capacity(4 * 1024 * 1024, file);
    let mut zip = ZipArchive::new(reader)?;
    zip.extract(target_dir)?;
    Ok(())
}
