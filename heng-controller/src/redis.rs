use crate::config::Config;

use anyhow::Result;
use mobc_redis::mobc;
use mobc_redis::redis;
use mobc_redis::RedisConnectionManager;

pub type Connection = mobc::Connection<RedisConnectionManager>;

pub struct Redis {
    pool: mobc::Pool<RedisConnectionManager>,
}

impl Redis {
    pub fn new() -> Result<Self> {
        let config = Config::global();
        let redis_url = config.redis.url.as_str();
        let max_open = config.redis.max_open;
        let client = redis::Client::open(redis_url)?;
        let mgr = RedisConnectionManager::new(client);
        let pool = mobc::Pool::builder().max_open(max_open).build(mgr);
        Ok(Self { pool })
    }

    pub async fn get_key_prefix(&self) -> &str {
        let config = Config::global();
        &config.redis.key_prefix
    }

    pub async fn get_connection(&self) -> Result<Connection> {
        Ok(self.pool.get().await?)
    }
}
