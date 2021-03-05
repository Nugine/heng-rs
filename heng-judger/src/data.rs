use crate::redis::RedisModule;
use crate::Config;

use heng_protocol::common as hp_common;
use heng_utils::crypto::{hex_sha256, is_hex_sha256_format, to_hex_string};
use hp_common::File;

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
use tracing::{error, warn};
use zip::ZipArchive;

pub struct DataModule {
    redis_module: Arc<RedisModule>,
    directory: PathBuf,
    download_size_limit: u64,
}

fn unzip(file_path: &Path, target_dir: &Path) -> Result<()> {
    let file = fs::File::open(file_path)?;
    let reader = BufReader::with_capacity(4 * 1024 * 1024, file);
    let mut zip = ZipArchive::new(reader)?;
    zip.extract(target_dir)?;
    Ok(())
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

    pub async fn download_file(&self, file: &File, path: &Path) -> Result<()> {
        let size_limit = self.download_size_limit;
        let (hashsum, content_hash) = match *file {
            File::Url {
                ref url,
                ref hashsum,
            } => {
                let res = reqwest::get(url).await?;
                if !res.status().is_success() {
                    anyhow::bail!("request failed: status = {}", res.status());
                }

                let mut stream = res.bytes_stream();
                let mut file = fs::File::create(&path)?;
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

                let content_hash = to_hex_string(hasher.finalize().as_ref());
                (hashsum, content_hash)
            }
            File::Direct {
                ref content,
                ref hashsum,
                base64,
            } => {
                let base64_decoded;
                let content_bytes = if base64 {
                    base64_decoded = base64::decode(content)?;
                    &*base64_decoded
                } else {
                    content.as_bytes()
                };
                let content_hash = hex_sha256(content_bytes);
                fs::write(path, content_bytes)?;

                (hashsum, content_hash)
            }
        };

        if let Some(hashsum) = hashsum {
            if content_hash != *hashsum {
                error!(?content_hash, expected=?hashsum,"file hashsum mismatch");
                anyhow::bail!("file hashsum mismatch");
            }
        }

        Ok(())
    }

    pub async fn load_data(&self, file: &File) -> Result<PathBuf> {
        let hashsum = match file {
            File::Url { ref hashsum, .. } => hashsum.as_deref(),
            File::Direct { ref hashsum, .. } => hashsum.as_deref(),
        };

        let generated_name;
        let data_name = match hashsum {
            Some(h) => {
                if !is_hex_sha256_format(h) {
                    error!(?h, "invalid file hashsum");
                    anyhow::bail!("invalid file hashsum")
                }
                h
            }
            None => {
                let timestamp = Utc::now().timestamp_nanos();
                let rng = rand::thread_rng().gen_range(0..1000);
                generated_name = format!("{}-{:03}", timestamp, rng);
                &generated_name
            }
        };

        let dir_path = self.directory.join(data_name);
        if dir_path.exists() {
            return Ok(dir_path);
        }

        let zip_path = scopeguard::guard(
            self.directory.join(format!("{}.zip", data_name)),
            |zip_path| {
                if let Err(err) = fs::remove_file(&zip_path) {
                    warn!(zip_path = %zip_path.display(), %err, "failed to remove zip file");
                }
            },
        );

        self.download_file(&file, &*zip_path).await?;
        unzip(&zip_path, &dir_path).context("failed to unzip")?;

        Ok(dir_path)
    }
}
