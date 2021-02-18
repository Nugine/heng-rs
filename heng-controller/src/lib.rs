mod config;
mod error_code;
mod errors;
mod judger;
mod redis;
mod routes;

pub use self::config::Config;
use self::judger::JudgerModule;
use self::redis::RedisModule;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use warp::reply::{self, Response};
use warp::{Filter, Rejection, Reply};

pub struct App {
    config: Config,
    redis: RedisModule,
    judger: Arc<JudgerModule>,
}

impl App {
    pub async fn new(config: Config) -> Result<Arc<Self>> {
        let redis = RedisModule::new(&config)?;
        let judger = JudgerModule::new(&config)?;
        let app = Self {
            config,
            redis,
            judger,
        };
        Ok(Arc::new(app))
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        let addr = self.config.server.address.parse::<SocketAddr>()?;
        let server = warp::serve(routes::routes(self));
        server.bind(addr).await;
        Ok(())
    }
}
