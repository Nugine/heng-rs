use crate::config::Config;

use actix_web::web;
use anyhow::Result;
use mobc_redis::mobc;
use mobc_redis::redis;
use mobc_redis::RedisConnectionManager;
use tracing::info;

pub type Connection = mobc::Connection<RedisConnectionManager>;
pub use redis::aio::ConnectionLike;

pub fn register() -> Result<impl Fn(&mut web::ServiceConfig) + Clone> {
    info!("initializing redis module");
    let state = web::Data::new(RedisModule::new()?);
    info!("redis module is initialized");

    Ok(move |cfg: &mut web::ServiceConfig| {
        cfg.app_data(state.clone());
    })
}

pub struct RedisModule {
    pool: mobc::Pool<RedisConnectionManager>,
}

impl RedisModule {
    pub fn new() -> Result<Self> {
        let config = Config::global();
        let redis_url = config.redis.url.as_str();
        let max_open = config.redis.max_open;
        let client = redis::Client::open(redis_url)?;
        let mgr = RedisConnectionManager::new(client);
        let pool = mobc::Pool::builder().max_open(max_open).build(mgr);
        Ok(Self { pool })
    }

    pub fn get_key_prefix(&self) -> &str {
        let config = Config::global();
        &config.redis.key_prefix
    }

    pub async fn get_connection(&self) -> Result<Connection> {
        Ok(self.pool.get().await?)
    }
}
