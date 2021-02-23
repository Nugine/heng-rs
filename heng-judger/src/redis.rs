use crate::config::Config;

use anyhow::Result;
use mobc_redis::mobc;
use mobc_redis::redis;
use mobc_redis::RedisConnectionManager;

pub type Connection = mobc::Connection<RedisConnectionManager>;
pub use redis::aio::ConnectionLike;

pub struct RedisModule {
    pool: mobc::Pool<RedisConnectionManager>,
}

impl RedisModule {
    pub fn new(config: &Config) -> Result<Self> {
        let redis_url = config.redis.url.as_str();
        let max_open = config.redis.max_open;
        let client = redis::Client::open(redis_url)?;
        let mgr = RedisConnectionManager::new(client);
        let pool = mobc::Pool::builder().max_open(max_open).build(mgr);
        Ok(Self { pool })
    }

    pub async fn get_connection(&self) -> Result<Connection> {
        Ok(self.pool.get().await?)
    }
}
